use std::collections::HashMap;

use async_openai::config::OpenAIConfig;
use async_openai::Client;
use chrono::Utc;
use futures::StreamExt;
use secrecy::ExposeSecret;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, info, instrument};

use crate::config::OpenAiConfig;
use crate::domain::models::auth_user::AuthUser;
use crate::domain::models::chat::ChatId;
use crate::domain::models::messages::{Message, MessageId, MessagePart};
use crate::domain::models::sampling::preset_by_name;
use crate::domain::models::scenario::{Round, Scenario, ScenarioId, ScenarioStatus};
use crate::domain::models::search_params::SearchParams;
use crate::domain::models::source::Source;
use crate::domain::models::stream_event::{FinishReason, MessageStreamEvent};
use crate::domain::models::token_usage::TokenUsage;
use crate::domain::models::tool::LLMTool;
use crate::domain::models::user::UserId;
use crate::errors::AppError;
use crate::infra::clients::llm_client::LlmClient;
use crate::operation_result::OperationResult;
use crate::services::knowledge_service::KnowledgeService;
use crate::services::prompts::{KbContext, PromptRenderer, TemplateContext};
use crate::services::scenario_service::ScenarioService;
use crate::services::scenarios::{create_tool_projectors, create_tools, scenario_by_name, simple};
use crate::services::search_service::SearchService;
use crate::services::user_service::UserService;

/// How many most recent past scenarios (turns) are included in the LLM history.
const PAST_HISTORY_LIMIT: usize = 5;

pub enum StreamRequest {
    Submit {
        user_message: Message,
        model: String,
    },
    Edit {
        user_message: Message,
        scenario_id: ScenarioId,
    },
    Regenerate {
        scenario_id: ScenarioId,
    },
    Continue,
}

#[derive(Clone)]
pub struct AiService {
    llm_client: LlmClient,
    prompt_renderer: PromptRenderer,
    scenario_service: ScenarioService,
    search_service: SearchService,
    knowledge_service: KnowledgeService,
    user_service: UserService,
    search_params: SearchParams,
    assistant_name: String,
}

impl AiService {
    pub fn new(
        config: &OpenAiConfig,
        prompt_renderer: PromptRenderer,
        scenario_service: ScenarioService,
        search_service: SearchService,
        knowledge_service: KnowledgeService,
        user_service: UserService,
        search_params: SearchParams,
        assistant_name: String,
    ) -> Self {
        let openai_config = OpenAIConfig::new()
            .with_api_base(config.base_url())
            .with_api_key(config.api_key().expose_secret());

        let llm_client = LlmClient::new(
            Client::with_config(openai_config),
            config.model(),
            config.include_usage(),
        );

        Self {
            llm_client,
            prompt_renderer,
            scenario_service,
            search_service,
            knowledge_service,
            user_service,
            search_params,
            assistant_name,
        }
    }

    #[instrument(skip(self, request, auth_user))]
    pub async fn stream(
        &self,
        chat_id: ChatId,
        auth_user: AuthUser,
        request: StreamRequest,
    ) -> Result<
        (
            ScenarioId,
            MessageId,
            String,
            Box<dyn futures::Stream<Item = MessageStreamEvent> + Send + Unpin>,
        ),
        AppError,
    > {
        let user = self.user_service.get_user(auth_user.user_id).await?;
        let user_name = user.name.clone().unwrap_or_else(|| user.username.clone());

        let result = match request {
            StreamRequest::Submit {
                user_message,
                model,
            } => Ok(self.stream_new(chat_id, &model, user_message, user_name)),
            StreamRequest::Edit {
                user_message,
                scenario_id,
            } => {
                let scenario = self
                    .scenario_service
                    .get_scenario(chat_id, scenario_id)
                    .await?
                    .ok_or_else(|| AppError::NotFound("Scenario not found".to_string()))?;
                self.rerun_scenario(chat_id, scenario, user_message, user_name)
                    .await
            },
            StreamRequest::Regenerate { scenario_id } => {
                let scenario = self
                    .scenario_service
                    .get_scenario(chat_id, scenario_id)
                    .await?
                    .ok_or_else(|| AppError::NotFound("Scenario not found".to_string()))?;
                let user_message = scenario
                    .user_message()
                    .cloned()
                    .ok_or_else(|| AppError::NotFound("User message not found".to_string()))?;
                self.rerun_scenario(chat_id, scenario, user_message, user_name)
                    .await
            },
            StreamRequest::Continue => {
                let scenarios = self
                    .scenario_service
                    .get_scenarios_for_chat(chat_id)
                    .await?;
                let scenario = scenarios
                    .into_iter()
                    .rev()
                    .find(|s| !s.is_complete())
                    .ok_or_else(|| {
                        AppError::BadRequest("No incomplete scenario to continue".to_string())
                    })?;
                Ok(self.stream_continue(chat_id, scenario, user_name))
            },
        };
        match &result {
            Ok(_) => info!(status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    async fn rerun_scenario(
        &self,
        chat_id: ChatId,
        scenario: Scenario,
        user_message: Message,
        user_name: String,
    ) -> Result<
        (
            ScenarioId,
            MessageId,
            String,
            Box<dyn futures::Stream<Item = MessageStreamEvent> + Send + Unpin>,
        ),
        AppError,
    > {
        let model = scenario.name;
        self.scenario_service.delete_scenario(scenario.id).await?;
        self.scenario_service
            .delete_scenarios_after(chat_id, scenario.created_at)
            .await?;
        Ok(self.stream_new(chat_id, &model, user_message, user_name))
    }

    #[instrument(skip(self, user_message, user_name))]
    pub fn stream_new(
        &self,
        chat_id: ChatId,
        model: &str,
        user_message: Message,
        user_name: String,
    ) -> (
        ScenarioId,
        MessageId,
        String,
        Box<dyn futures::Stream<Item = MessageStreamEvent> + Send + Unpin>,
    ) {
        let definition = scenario_by_name(model).unwrap_or_else(simple);
        let scenario = Scenario::new(
            chat_id,
            definition.name.to_string(),
            ScenarioStatus::Running,
            user_message,
        );
        let assistant_message_id = MessageId::new();
        let result = self.spawn_scenario(
            chat_id,
            model.to_string(),
            scenario,
            assistant_message_id,
            user_name,
            true,
            None,
            None,
        );
        result
    }

    #[instrument(skip(self, scenario), fields(
        scenario_id = %scenario.id.0,
        name = %scenario.name,
        status = %scenario.status.as_str(),
        rounds = scenario.rounds.len()
    ))]
    pub fn stream_continue(
        &self,
        chat_id: ChatId,
        scenario: Scenario,
        user_name: String,
    ) -> (
        ScenarioId,
        MessageId,
        String,
        Box<dyn futures::Stream<Item = MessageStreamEvent> + Send + Unpin>,
    ) {
        let assistant_message_id = scenario
            .rounds
            .get(1)
            .map(|r| r.message.id)
            .unwrap_or_else(MessageId::new);
        let model = scenario.name.clone();
        let result = self.spawn_scenario(
            chat_id,
            model,
            scenario,
            assistant_message_id,
            user_name,
            false,
            None,
            None,
        );
        result
    }

    /// Run a scenario to completion and return the finished scenario. The caller must have
    /// already created the chat (create_chat_shell); the scenario is inserted here.
    #[instrument(skip(self, question, history, client_system), fields(chat_id = %chat_id.0))]
    pub async fn complete(
        &self,
        chat_id: ChatId,
        model: &str,
        question: Message,
        history: Vec<Message>,
        client_system: Option<String>,
        user_id: UserId,
    ) -> Result<Scenario, AppError> {
        let user = self.user_service.get_user(user_id).await?;
        let user_name = user.name.clone().unwrap_or_else(|| user.username.clone());

        let scenario = Scenario::new(
            chat_id,
            model.to_string(),
            ScenarioStatus::Running,
            question,
        );
        let scenario_id = scenario.id;
        let assistant_message_id = MessageId::new();

        let (_, _, _, mut stream) = self.spawn_scenario(
            chat_id,
            model.to_string(),
            scenario,
            assistant_message_id,
            user_name,
            true,
            Some(history),
            client_system,
        );

        while let Some(event) = stream.next().await {
            if let MessageStreamEvent::Error { error_text } = event {
                info!(scenario_id = %scenario_id.0, error = error_text, status = "failed");
                return Err(AppError::InternalServerError);
            }
        }

        let result = match self
            .scenario_service
            .get_scenario(chat_id, scenario_id)
            .await?
        {
            Some(scenario) if scenario.status == ScenarioStatus::Completed => Ok(scenario),
            Some(_) => Err(AppError::InternalServerError),
            None => Err(AppError::NotFound("Scenario not found".into())),
        };
        match &result {
            Ok(scenario) => info!(scenario_id = %scenario.id.0, status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    fn spawn_scenario(
        &self,
        chat_id: ChatId,
        model: String,
        scenario: Scenario,
        assistant_message_id: MessageId,
        user_name: String,
        is_new: bool,
        provided_history: Option<Vec<Message>>,
        client_system: Option<String>,
    ) -> (
        ScenarioId,
        MessageId,
        String,
        Box<dyn futures::Stream<Item = MessageStreamEvent> + Send + Unpin>,
    ) {
        let scenario_id = scenario.id;
        let scenario_name = scenario.name.clone();

        let llm_client = self.llm_client.clone();
        let prompt_renderer = self.prompt_renderer.clone();
        let scenario_service = self.scenario_service.clone();
        let search_service = self.search_service.clone();
        let knowledge_service = self.knowledge_service.clone();
        let search_params = self.search_params;
        let assistant_name = self.assistant_name.clone();

        let (tx, rx) = mpsc::channel(64);
        let error_tx = tx.clone();

        tokio::spawn(async move {
            if let Err(e) = run_scenario(
                llm_client,
                prompt_renderer,
                scenario_service,
                search_service,
                knowledge_service,
                search_params,
                assistant_name,
                chat_id,
                &model,
                scenario,
                assistant_message_id,
                user_name,
                is_new,
                provided_history,
                client_system,
                tx,
            )
            .await
            {
                let _ = error_tx
                    .send(MessageStreamEvent::Error {
                        error_text: e.to_string(),
                    })
                    .await;
            }
        });

        (
            scenario_id,
            assistant_message_id,
            scenario_name,
            Box::new(ReceiverStream::new(rx)),
        )
    }
}

async fn run_scenario(
    llm_client: LlmClient,
    prompt_renderer: PromptRenderer,
    scenario_service: ScenarioService,
    search_service: SearchService,
    knowledge_service: KnowledgeService,
    search_params: SearchParams,
    assistant_name: String,
    chat_id: ChatId,
    model: &str,
    mut scenario: Scenario,
    assistant_message_id: MessageId,
    user_name: String,
    is_new: bool,
    provided_history: Option<Vec<Message>>,
    client_system: Option<String>,
    tx: mpsc::Sender<MessageStreamEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let definition = scenario_by_name(model).unwrap_or_else(simple);

    let scenario_sampling = definition
        .sampling_preset
        .and_then(preset_by_name)
        .unwrap_or_default();

    let kbs = knowledge_service
        .list_kbs()
        .await
        .map_err(|e| format!("Failed to list knowledge bases: {e}"))?;

    let question = scenario.rounds[0].message.text();
    let ctx = TemplateContext {
        assistant_name,
        user_name,
        current_date: Utc::now().format("%Y-%m-%d").to_string(),
        question,
        knowledge_bases: kbs.iter().map(KbContext::from).collect(),
        client_system,
    };

    let global_prompt = prompt_renderer
        .render_global_system(&ctx)
        .map_err(|e| format!("Failed to render global system prompt: {e}"))?;

    debug!(global_prompt, "rendered global prompt");

    // Finalize scenario identity.
    scenario.name = definition.name.to_string();
    scenario.status = ScenarioStatus::Running;

    // Dialog history. A provided history (OpenAI API) is used verbatim.
    // For a NEW scenario, load history BEFORE inserting so the just-inserted
    // scenario is not double-counted (its question lives in the round prompts).
    // For a continue, load it AFTER update so the current scenario's
    // already-stored rounds are reflected in the history.
    let mut history: Vec<Message> = match provided_history {
        Some(history) => {
            scenario = scenario_service
                .insert_scenario(&scenario)
                .await
                .map_err(|e| format!("Failed to save scenario: {e}"))?;
            history
        },
        None if is_new => {
            let scenarios = scenario_service
                .get_scenarios_for_chat(chat_id)
                .await
                .map_err(|e| format!("Failed to load chat history: {e}"))?;
            scenario = scenario_service
                .insert_scenario(&scenario)
                .await
                .map_err(|e| format!("Failed to save scenario: {e}"))?;
            build_history(scenarios, None)
        },
        None => {
            scenario = scenario_service
                .update_scenario(&scenario)
                .await
                .map_err(|e| format!("Failed to update scenario: {e}"))?;
            let scenarios = scenario_service
                .get_scenarios_for_chat(chat_id)
                .await
                .map_err(|e| format!("Failed to load chat history: {e}"))?;
            build_history(scenarios, Some(scenario.id))
        },
    };

    let tool_projectors =
        create_tool_projectors(&search_service, &knowledge_service, search_params);
    project_history_tools(&mut history, &tool_projectors);

    let next_round = scenario.rounds.len().saturating_sub(1); // assistant rounds already stored

    for round_index in next_round..definition.rounds.len() {
        let round_def = &definition.rounds[round_index];
        debug!(
            round_index,
            prompt_template = %round_def.prompt_template,
            tool_names = ?round_def.tool_names,
            max_llm_calls = round_def.max_llm_calls,
            reasoning_effort = ?round_def.reasoning_effort,
            sampling = ?scenario_sampling.merge(&round_def.sampling),
            status = "round_start"
        );

        let round_prompt = prompt_renderer
            .render(&round_def.prompt_template, &ctx)
            .map_err(|e| {
                format!(
                    "Failed to render prompt '{}': {e}",
                    round_def.prompt_template
                )
            })?;

        history.push(Message::user(chat_id, round_prompt));

        let tools = create_tools(
            &round_def.tool_names,
            &search_service,
            &knowledge_service,
            search_params,
        );

        let params = round_def.to_run_params(
            global_prompt.clone(),
            history.clone(),
            tools,
            scenario_sampling,
            round_index,
            assistant_message_id,
        );

        let mut round_stream = llm_client.execute_round(chat_id, params);
        let mut round_message: Option<Message> = None;
        let mut round_sources: Vec<Source> = Vec::new();
        let mut round_usage: TokenUsage = TokenUsage::default();

        while let Some(event) = round_stream.next().await {
            if let MessageStreamEvent::SourceUrl(source) = &event {
                round_sources.push(source.clone());
            }
            if let MessageStreamEvent::LlmCallFinish {
                finish_reason,
                ref message,
                usage,
            } = &event
            {
                if let Some(u) = usage {
                    round_usage.add(u);
                }
                if let FinishReason::Stop = finish_reason {
                    round_message = Some(message.clone());
                }
            }
            if tx.send(event).await.is_err() {
                return Ok(());
            }
        }

        let Some(mut msg) = round_message else {
            scenario.status = ScenarioStatus::Failed;
            let _ = scenario_service.update_scenario(&scenario).await;
            return Ok(());
        };

        let round = Round {
            message: msg.clone(),
            sources: round_sources,
            hidden: round_def.hidden,
            usage: if round_usage == TokenUsage::default() {
                None
            } else {
                Some(round_usage.clone())
            },
        };
        let idx = scenario.rounds.len() as i32;
        // Persist incrementally under an optimistic lock on updated_at; a concurrent
        // tab / regenerate bumping the scenario makes this fail, aborting the run
        // before the final update_scenario can clobber the winner's rounds.
        scenario.updated_at = scenario_service
            .insert_round(scenario.id, idx, &round, scenario.updated_at)
            .await
            .map_err(|e| format!("Failed to save round: {e}"))?;
        // The scenario total is the sum of its rounds; rounds without reported
        // usage contribute nothing, so `None` stays `None` until the first
        // reported round.
        if let Some(u) = &round.usage {
            match scenario.usage.take() {
                Some(mut acc) => {
                    acc.add(u);
                    scenario.usage = Some(acc);
                },
                None => scenario.usage = Some(u.clone()),
            }
        }
        scenario.rounds.push(round);
        project_history_tools(std::slice::from_mut(&mut msg), &tool_projectors);
        history.push(msg);
    }

    scenario.status = ScenarioStatus::Completed;
    if let Err(e) = scenario_service.update_scenario(&scenario).await {
        tracing::error!(
            chat_id = ?chat_id,
            error = %e,
            status = "update_scenario_failed"
        );
    }

    Ok(())
}

/// Build the LLM history from chat scenarios. The current scenario (if given)
/// is included in full — its hidden rounds carry the tool outputs that later
/// rounds of the same scenario depend on. Past scenarios contribute only their
/// visible rounds, trimmed to text parts, capped to the most recent
/// `PAST_HISTORY_LIMIT`.
fn build_history(
    scenarios: Vec<Scenario>,
    current_scenario_id: Option<ScenarioId>,
) -> Vec<Message> {
    let current = current_scenario_id.and_then(|id| scenarios.iter().find(|s| s.id == id));

    let mut history = Vec::new();
    let past: Vec<&Scenario> = scenarios
        .iter()
        .filter(|s| Some(s.id) != current_scenario_id)
        .rev()
        .take(PAST_HISTORY_LIMIT)
        .collect();
    for scenario in past.into_iter().rev() {
        for round in &scenario.rounds {
            if round.hidden {
                continue;
            }
            history.push(text_only_message(&round.message));
        }
    }

    if let Some(scenario) = current {
        for round in &scenario.rounds {
            history.push(round.message.clone());
        }
    }

    history
}

/// A copy of the message keeping only its text parts (reasoning, sources and
/// tool calls dropped).
fn text_only_message(message: &Message) -> Message {
    let parts = message
        .parts
        .iter()
        .filter(|part| matches!(part, MessagePart::Text(_)))
        .cloned()
        .collect();
    Message {
        parts,
        ..message.clone()
    }
}

/// Replace stored tool outputs with their LLM-facing projection so the model
/// sees the same view as during live execution. Unknown tools are left as-is.
fn project_history_tools(history: &mut [Message], projectors: &HashMap<String, Box<dyn LLMTool>>) {
    for message in history.iter_mut() {
        for part in message.parts.iter_mut() {
            if let MessagePart::ToolCall(tc) = part {
                if let (Some(output), Some(tool)) =
                    (tc.output.clone(), projectors.get(&tc.tool_name))
                {
                    tc.output = Some(tool.llm_output(&output));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::chat::ChatId;
    use crate::domain::models::messages::{Role, TextBlock, ToolCallBlock, ToolCallState};
    use crate::domain::models::tool::{LLMTool, ToolExecution};
    use async_openai::types::chat::ChatCompletionTools;
    use async_trait::async_trait;
    use serde_json::{json, Value};

    fn chat_id() -> ChatId {
        ChatId(uuid::Uuid::new_v4())
    }

    fn user_msg(text: &str) -> Message {
        Message::user(chat_id(), text.to_string())
    }

    fn assistant_msg(text: &str) -> Message {
        let mut msg = Message::new(chat_id(), Role::Assistant);
        msg.parts.push(MessagePart::Text(TextBlock {
            text: text.to_string(),
        }));
        msg
    }

    fn hidden_tool_round() -> Round {
        let mut msg = Message::new(chat_id(), Role::Assistant);
        msg.parts.push(MessagePart::ToolCall(ToolCallBlock {
            tool_call_id: "call_1".to_string(),
            tool_name: "search_in_all_kbs".to_string(),
            input: json!({ "query": "q" }),
            output: Some(json!("found documents")),
            error: None,
            state: ToolCallState::Done,
        }));
        Round {
            message: msg,
            sources: Vec::new(),
            hidden: true,
            usage: None,
        }
    }

    /// Scenario: user question, hidden tool round, visible answer.
    fn scenario(question: &str, answer: &str) -> Scenario {
        let mut scenario = Scenario::new(
            chat_id(),
            "simple".to_string(),
            ScenarioStatus::Completed,
            user_msg(question),
        );
        scenario.rounds.push(hidden_tool_round());
        scenario.rounds.push(Round {
            message: assistant_msg(answer),
            sources: Vec::new(),
            hidden: false,
            usage: None,
        });
        scenario
    }

    fn texts(messages: &[Message]) -> Vec<String> {
        messages.iter().map(|m| m.text()).collect()
    }

    #[test]
    fn past_scenarios_keep_only_visible_rounds() {
        let scenarios = vec![scenario("q1", "a1"), scenario("q2", "a2")];
        let history = build_history(scenarios, None);
        let expected: Vec<String> = ["q1", "a1", "q2", "a2"]
            .into_iter()
            .map(str::to_string)
            .collect();
        assert_eq!(texts(&history), expected);
    }

    #[test]
    fn non_text_parts_are_dropped_from_past_scenarios() {
        let scenarios = vec![scenario("q1", "a1")];
        let history = build_history(scenarios, None);
        assert!(history
            .iter()
            .all(|m| m.parts.iter().all(|p| matches!(p, MessagePart::Text(_)))));
    }

    #[test]
    fn current_scenario_is_kept_in_full() {
        let current = scenario("q2", "a2");
        let current_id = current.id;
        let scenarios = vec![scenario("q1", "a1"), current];
        let history = build_history(scenarios, Some(current_id));

        let expected: Vec<String> = ["q1", "a1", "q2", "", "a2"]
            .into_iter()
            .map(str::to_string)
            .collect();
        assert_eq!(texts(&history), expected);
        let hidden_msg = &history[3];
        assert!(hidden_msg
            .parts
            .iter()
            .any(|p| matches!(p, MessagePart::ToolCall(_))));
    }

    #[test]
    fn past_scenarios_are_capped_to_the_most_recent_limit() {
        let scenarios: Vec<Scenario> = (0..7)
            .map(|i| scenario(&format!("q{i}"), &format!("a{i}")))
            .collect();
        let history = build_history(scenarios, None);
        let expected: Vec<String> = (2..7)
            .flat_map(|i| [format!("q{i}"), format!("a{i}")])
            .collect();
        assert_eq!(texts(&history), expected);
    }

    #[test]
    fn scenario_with_only_user_round_contributes_just_the_question() {
        let bare = Scenario::new(
            chat_id(),
            "simple".to_string(),
            ScenarioStatus::Running,
            user_msg("q1"),
        );
        assert_eq!(bare.rounds.len(), 1);

        let history = build_history(vec![bare], None);
        let expected: Vec<String> = vec!["q1".to_string()];
        assert_eq!(texts(&history), expected);
    }

    #[test]
    fn empty_history() {
        assert!(build_history(Vec::new(), None).is_empty());
    }

    struct ProjectingTool;

    #[async_trait]
    impl LLMTool for ProjectingTool {
        fn definition(&self) -> ChatCompletionTools {
            unreachable!()
        }

        async fn execute(&mut self, _arguments: &str) -> Result<ToolExecution, String> {
            unreachable!()
        }

        fn llm_output(&self, output: &Value) -> Value {
            let mut projected = output.clone();
            projected["projected"] = json!(true);
            projected
        }
    }

    fn assistant_msg_with_tool(tool_name: &str, output: Value) -> Message {
        let mut msg = Message::new(chat_id(), Role::Assistant);
        msg.parts.push(MessagePart::ToolCall(ToolCallBlock {
            tool_call_id: "call_1".to_string(),
            tool_name: tool_name.to_string(),
            input: json!({}),
            output: Some(output),
            error: None,
            state: ToolCallState::Done,
        }));
        msg
    }

    fn tool_call_of(message: &Message) -> &ToolCallBlock {
        match &message.parts[0] {
            MessagePart::ToolCall(tc) => tc,
            _ => panic!("expected a tool call part"),
        }
    }

    #[test]
    fn project_history_tools_projects_known_tools_only() {
        let mut history = vec![
            assistant_msg_with_tool("search_in_all_kbs", json!({ "a": 1 })),
            assistant_msg_with_tool("unknown_tool", json!({ "a": 1 })),
        ];

        let mut projectors: HashMap<String, Box<dyn LLMTool>> = HashMap::new();
        projectors.insert("search_in_all_kbs".to_string(), Box::new(ProjectingTool));
        project_history_tools(&mut history, &projectors);

        let known = tool_call_of(&history[0]);
        assert_eq!(known.output.as_ref().unwrap()["a"], json!(1));
        assert_eq!(known.output.as_ref().unwrap()["projected"], json!(true));

        let unknown = tool_call_of(&history[1]);
        assert_eq!(unknown.output.as_ref().unwrap(), &json!({ "a": 1 }));
        assert!(unknown.output.as_ref().unwrap().get("projected").is_none());
    }
}
