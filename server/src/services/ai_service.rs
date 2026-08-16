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
use crate::domain::models::messages::{Message, MessageId, MessagePart, Role, TextBlock};
use crate::domain::models::stream_event::MessageStreamEvent;
use crate::infra::clients::llm_client::{LlmClient, RunRoundParams};
use crate::services::knowledge_service::KnowledgeService;
use crate::services::message_service::MessageService;
use crate::services::prompts::{PromptRenderer, TemplateContext};
use crate::services::scenarios::{create_tools, scenario_by_name, simple};
use crate::services::search_service::SearchService;
use uuid::Uuid;

#[derive(Clone)]
pub struct AiService {
    llm_client: LlmClient,
    prompt_renderer: PromptRenderer,
    message_service: MessageService,
    search_service: SearchService,
    knowledge_service: KnowledgeService,
}

impl AiService {
    pub fn new(
        config: &OpenAiConfig,
        prompt_renderer: PromptRenderer,
        message_service: MessageService,
        search_service: SearchService,
        knowledge_service: KnowledgeService,
    ) -> Self {
        let openai_config = OpenAIConfig::new()
            .with_api_base(config.base_url())
            .with_api_key(config.api_key().expose_secret());

        let llm_client = LlmClient::new(Client::with_config(openai_config), config.model());

        Self {
            llm_client,
            prompt_renderer,
            message_service,
            search_service,
            knowledge_service,
        }
    }

    #[instrument(skip(self, user_message))]
    pub fn stream(
        &self,
        chat_id: ChatId,
        model: &str,
        user_message: &str,
        save_user_message: bool,
    ) -> (
        MessageId,
        impl futures::Stream<Item = MessageStreamEvent> + Send,
    ) {
        let llm_client = self.llm_client.clone();
        let prompt_renderer = self.prompt_renderer.clone();
        let message_service = self.message_service.clone();
        let search_service = self.search_service.clone();
        let knowledge_service = self.knowledge_service.clone();
        let chat_id2 = chat_id;
        let model = model.to_string();
        let user_message = user_message.to_string(); // TODO: Fix user message only, take previous dialog, about 3-4 messages of assistant and user
        let assistant_id = MessageId(Uuid::new_v4());

        let (tx, rx) = mpsc::channel(64);

        tokio::spawn(async move {
            if let Err(e) = run_scenario(
                llm_client,
                prompt_renderer,
                message_service,
                search_service,
                knowledge_service,
                chat_id2,
                &model,
                &user_message,
                save_user_message,
                assistant_id,
                &tx,
            )
            .await
            {
                let _ = tx
                    .send(MessageStreamEvent::Error {
                        error_text: e.to_string(),
                    })
                    .await;
            }
        });

        (assistant_id, ReceiverStream::new(rx))
    }
}

async fn run_scenario(
    llm_client: LlmClient,
    prompt_renderer: PromptRenderer,
    message_service: MessageService,
    search_service: SearchService,
    knowledge_service: KnowledgeService,
    chat_id: ChatId,
    model: &str,
    user_message: &str,
    save_user_message: bool,
    assistant_id: MessageId,
    tx: &mpsc::Sender<MessageStreamEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let scenario = scenario_by_name(model).unwrap_or_else(simple);

    // TODO Fix hardcoded template params, move general to config, other take from input params
    let ctx = TemplateContext {
        assistant_name: "Chiroo".into(),
        user_name: String::new(),
        current_date: Utc::now().format("%Y-%m-%d").to_string(),
        question: user_message.to_string(),
    };

    let global_prompt = prompt_renderer
        .render_global_system(&ctx)
        .map_err(|e| format!("Failed to render global system prompt: {e}"))?;

    let mut user_msg = Message::new(chat_id, Role::User);
    user_msg.parts.push(MessagePart::Text(TextBlock {
        text: user_message.to_string(),
    }));

    // Persisting the user's question is critical — the scenario is meaningless
    // without it, so a failed insert aborts the whole run.
    if save_user_message {
        message_service
            .insert_message(&user_msg)
            .await
            .map_err(|e| format!("Failed to save user message: {e}"))?;
    }

    let mut history: Vec<Message> = Vec::new();
    let last_round_index = scenario.rounds.len().saturating_sub(1);

    for (round_index, round) in scenario.rounds.clone().into_iter().enumerate() {
        debug!(
            round_index,
            prompt_template = %round.prompt_template,
            tool_names = ?round.tool_names,
            max_llm_calls = round.max_llm_calls,
            reasoning_effort = ?round.reasoning_effort,
            status = "round_start"
        );

        let round_prompt = prompt_renderer
            .render(&round.prompt_template, &ctx)
            .map_err(|e| format!("Failed to render prompt '{}': {e}", round.prompt_template))?;

        let mut round_user = Message::new(chat_id, Role::User);
        round_user
            .parts
            .push(MessagePart::Text(TextBlock { text: round_prompt }));
        history.push(round_user);

        let tools = create_tools(&round.tool_names, &search_service, &knowledge_service);

        let params = RunRoundParams {
            system_prompt: global_prompt.clone(),
            history: history.clone(),
            tools,
            max_llm_calls: round.max_llm_calls,
            reasoning_effort: round.reasoning_effort,
            extra_body: round.extra_body.clone(),
            tool_choice: round.tool_choice,
            preserve_thinking: round.enable_thinking && round.preserve_thinking,
            message_id_override: (round_index == last_round_index).then_some(assistant_id),
        };

        let mut round_stream = llm_client.execute_round(chat_id, params);
        let mut round_message = None;

        while let Some(event) = round_stream.next().await {
            if let MessageStreamEvent::LlmCallFinish {
                finish_reason: crate::domain::models::stream_event::FinishReason::Stop,
                ref message,
            } = event
            {
                round_message = Some(message.clone());
            }

            if tx.send(event).await.is_err() {
                return Ok(());
            }
        }
        // TODO: Accumulate all messages and write full round instead of one by one steps
        if let Some(msg) = round_message {
            let tool_calls = msg
                .parts
                .iter()
                .filter(|p| matches!(p, MessagePart::ToolCall(_)))
                .count();
            debug!(
                round_index,
                parts = msg.parts.len(),
                tool_calls,
                status = "round_end"
            );

            if let Err(e) = message_service.insert_message(&msg).await {
                tracing::error!(
                    chat_id = ?chat_id,
                    error = %e,
                    status = "insert_message_failed"
                );
            }
            history.push(msg);
        }
    }

    Ok(())
}
