use async_openai::config::OpenAIConfig;
use async_openai::Client;
use chrono::Utc;
use futures::StreamExt;
use secrecy::ExposeSecret;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, instrument};

use crate::config::OpenAiConfig;
use crate::domain::models::chat::ChatId;
use crate::domain::models::messages::{Message, MessageId};
use crate::domain::models::sampling::preset_by_name;
use crate::domain::models::scenario::{Round, Scenario, ScenarioId, ScenarioStatus};
use crate::domain::models::source::Source;
use crate::domain::models::stream_event::{FinishReason, MessageStreamEvent};
use crate::errors::AppError;
use crate::infra::clients::llm_client::LlmClient;
use crate::services::knowledge_service::KnowledgeService;
use crate::services::prompts::{KbContext, PromptRenderer, TemplateContext};
use crate::services::scenario_service::ScenarioService;
use crate::services::scenarios::{create_tools, scenario_by_name, simple};
use crate::services::search_service::SearchService;

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
    search_limit: usize,
}

impl AiService {
    pub fn new(
        config: &OpenAiConfig,
        prompt_renderer: PromptRenderer,
        scenario_service: ScenarioService,
        search_service: SearchService,
        knowledge_service: KnowledgeService,
        search_limit: usize,
    ) -> Self {
        let openai_config = OpenAIConfig::new()
            .with_api_base(config.base_url())
            .with_api_key(config.api_key().expose_secret());

        let llm_client = LlmClient::new(Client::with_config(openai_config), config.model());

        Self {
            llm_client,
            prompt_renderer,
            scenario_service,
            search_service,
            knowledge_service,
            search_limit,
        }
    }

    #[instrument(skip(self, request))]
    pub async fn stream(
        &self,
        chat_id: ChatId,
        request: StreamRequest,
    ) -> Result<
        (
            ScenarioId,
            MessageId,
            Box<dyn futures::Stream<Item = MessageStreamEvent> + Send + Unpin>,
        ),
        AppError,
    > {
        match request {
            StreamRequest::Submit {
                user_message,
                model,
            } => Ok(self.stream_new(chat_id, &model, user_message)),
            StreamRequest::Edit {
                user_message,
                scenario_id,
            } => {
                let scenario = self
                    .scenario_service
                    .get_scenario(chat_id, scenario_id)
                    .await?
                    .ok_or_else(|| AppError::NotFound("Scenario not found".to_string()))?;
                self.rerun_scenario(chat_id, scenario, user_message).await
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
                self.rerun_scenario(chat_id, scenario, user_message).await
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
                Ok(self.stream_continue(chat_id, scenario))
            },
        }
    }

    async fn rerun_scenario(
        &self,
        chat_id: ChatId,
        scenario: Scenario,
        user_message: Message,
    ) -> Result<
        (
            ScenarioId,
            MessageId,
            Box<dyn futures::Stream<Item = MessageStreamEvent> + Send + Unpin>,
        ),
        AppError,
    > {
        let model = scenario.name;
        self.scenario_service.delete_scenario(scenario.id).await?;
        self.scenario_service
            .delete_scenarios_after(chat_id, scenario.created_at)
            .await?;
        Ok(self.stream_new(chat_id, &model, user_message))
    }

    #[instrument(skip(self, user_message))]
    pub fn stream_new(
        &self,
        chat_id: ChatId,
        model: &str,
        user_message: Message,
    ) -> (
        ScenarioId,
        MessageId,
        Box<dyn futures::Stream<Item = MessageStreamEvent> + Send + Unpin>,
    ) {
        let definition = scenario_by_name(model).unwrap_or_else(simple);
        let scenario = Scenario::new(
            chat_id,
            definition.name.to_string(),
            ScenarioStatus::Running,
            user_message,
        );
        let scenario_id = scenario.id;
        let assistant_message_id = MessageId::new();

        let llm_client = self.llm_client.clone();
        let prompt_renderer = self.prompt_renderer.clone();
        let scenario_service = self.scenario_service.clone();
        let search_service = self.search_service.clone();
        let knowledge_service = self.knowledge_service.clone();
        let search_limit = self.search_limit;
        let model = model.to_string();

        let (tx, rx) = mpsc::channel(64);
        let error_tx = tx.clone();

        tokio::spawn(async move {
            if let Err(e) = run_scenario(
                llm_client,
                prompt_renderer,
                scenario_service,
                search_service,
                knowledge_service,
                search_limit,
                chat_id,
                &model,
                scenario,
                assistant_message_id,
                true,
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
            Box::new(ReceiverStream::new(rx)),
        )
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
    ) -> (
        ScenarioId,
        MessageId,
        Box<dyn futures::Stream<Item = MessageStreamEvent> + Send + Unpin>,
    ) {
        let scenario_id = scenario.id;
        let assistant_message_id = scenario
            .rounds
            .get(1)
            .map(|r| r.message.id)
            .unwrap_or_else(MessageId::new);

        let llm_client = self.llm_client.clone();
        let prompt_renderer = self.prompt_renderer.clone();
        let scenario_service = self.scenario_service.clone();
        let search_service = self.search_service.clone();
        let knowledge_service = self.knowledge_service.clone();
        let search_limit = self.search_limit;
        let model = scenario.name.clone();

        let (tx, rx) = mpsc::channel(64);
        let error_tx = tx.clone();

        tokio::spawn(async move {
            if let Err(e) = run_scenario(
                llm_client,
                prompt_renderer,
                scenario_service,
                search_service,
                knowledge_service,
                search_limit,
                chat_id,
                &model,
                scenario,
                assistant_message_id,
                false,
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
    search_limit: usize,
    chat_id: ChatId,
    model: &str,
    mut scenario: Scenario,
    assistant_message_id: MessageId,
    is_new: bool,
    tx: mpsc::Sender<MessageStreamEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let definition = scenario_by_name(model).unwrap_or_else(simple);

    let scenario_sampling = definition
        .sampling_preset
        .and_then(preset_by_name)
        .unwrap_or_default();

    let knowledge_bases = knowledge_service
        .list_kbs()
        .await
        .map_err(|e| format!("Failed to list knowledge bases: {e}"))?
        .iter()
        .map(KbContext::from)
        .collect::<Vec<_>>();

    let question = scenario.rounds[0].message.text();
    let ctx = TemplateContext {
        assistant_name: "Chiroo".into(),
        user_name: String::new(),
        current_date: Utc::now().format("%Y-%m-%d").to_string(),
        question,
        knowledge_bases,
    };

    let global_prompt = prompt_renderer
        .render_global_system(&ctx)
        .map_err(|e| format!("Failed to render global system prompt: {e}"))?;

    // Finalize scenario identity.
    scenario.name = definition.name.to_string();
    scenario.status = ScenarioStatus::Running;

    // Full dialog history. For a NEW scenario, get_history re-reads the DB and
    // would include the just-inserted scenario, so load history BEFORE inserting
    // to avoid double-counting the current user message. For a continue, load it
    // AFTER update so the updated scenario (with its already-stored rounds) is
    // reflected in the history.
    let mut history: Vec<Message> = if is_new {
        let history = scenario_service
            .get_history(chat_id)
            .await
            .map_err(|e| format!("Failed to load chat history: {e}"))?;
        scenario = scenario_service
            .insert_scenario(&scenario)
            .await
            .map_err(|e| format!("Failed to save scenario: {e}"))?;
        history
    } else {
        scenario = scenario_service
            .update_scenario(&scenario)
            .await
            .map_err(|e| format!("Failed to update scenario: {e}"))?;
        scenario_service
            .get_history(chat_id)
            .await
            .map_err(|e| format!("Failed to load chat history: {e}"))?
    };

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
            search_limit,
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

        while let Some(event) = round_stream.next().await {
            if let MessageStreamEvent::SourceUrl(source) = &event {
                round_sources.push(source.clone());
            }
            if let MessageStreamEvent::LlmCallFinish {
                finish_reason: FinishReason::Stop,
                ref message,
            } = event
            {
                round_message = Some(message.clone());
            }
            if tx.send(event).await.is_err() {
                return Ok(());
            }
        }

        let Some(msg) = round_message else {
            scenario.status = ScenarioStatus::Failed;
            let _ = scenario_service.update_scenario(&scenario).await;
            return Ok(());
        };

        let round = Round {
            message: msg.clone(),
            sources: round_sources,
        };
        let idx = scenario.rounds.len() as i32;
        // Persist incrementally under an optimistic lock on updated_at; a concurrent
        // tab / regenerate bumping the scenario makes this fail, aborting the run
        // before the final update_scenario can clobber the winner's rounds.
        scenario.updated_at = scenario_service
            .insert_round(scenario.id, idx, &round, scenario.updated_at)
            .await
            .map_err(|e| format!("Failed to save round: {e}"))?;
        scenario.rounds.push(round);
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
