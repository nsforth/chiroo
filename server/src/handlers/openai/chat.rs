use axum::extract::State;

use super::error::OpenAiApiError;
use super::messages::ChatCompletionRequest;
use super::response::ChatCompletionResponse;
use crate::domain::models::auth_user::AuthUser;
use crate::domain::models::messages::{Message, MessagePart, TextBlock};
use crate::services::scenarios::scenario_by_name;
use crate::state::AppState;

pub async fn create_chat_completion(
    State(state): State<AppState>,
    auth_user: AuthUser,
    json: Result<axum::Json<ChatCompletionRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<axum::Json<ChatCompletionResponse>, OpenAiApiError> {
    let request = json.map_err(|_| OpenAiApiError::bad_request("Invalid JSON body"))?;
    let converted = request.convert()?;
    let _definition = scenario_by_name(&request.model)
        .ok_or_else(|| OpenAiApiError::model_not_found(&request.model))?;

    let chat = state
        .chat_service
        .create_chat_shell(converted.question.clone(), auth_user.user_id)
        .await
        .map_err(OpenAiApiError::from)?;

    let question = Message::user(chat.id, converted.question);
    let history: Vec<Message> = converted
        .history
        .into_iter()
        .map(|(role, text)| {
            let mut message = Message::new(chat.id, role);
            message.parts.push(MessagePart::Text(TextBlock { text }));
            message
        })
        .collect();

    let scenario = match state
        .ai_service
        .complete(
            chat.id,
            &request.model,
            question,
            history,
            converted.client_system,
            auth_user.user_id,
        )
        .await
    {
        Ok(scenario) => scenario,
        Err(error) => {
            // A failed run must not leave an empty chat behind: the UI requires
            // at least one scenario per chat. Drop the shell only if no scenario
            // was persisted (a partially persisted scenario is a normal UI state).
            let has_scenario = state
                .scenario_service
                .get_scenarios_for_chat(chat.id)
                .await
                .map(|scenarios| !scenarios.is_empty())
                .unwrap_or(true);
            if !has_scenario {
                let _ = state
                    .chat_service
                    .delete_chat(&chat.id, &auth_user.user_id)
                    .await;
            }
            return Err(OpenAiApiError::from(error));
        },
    };

    Ok(axum::Json(ChatCompletionResponse::from_scenario(
        &scenario,
    )?))
}
