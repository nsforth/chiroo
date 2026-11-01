use axum::extract::Path;
use axum::Json;
use serde::Serialize;

use super::error::OpenAiApiError;
use crate::domain::models::api_principal::ApiPrincipal;
use crate::services::scenarios::{list_scenarios, scenario_by_name};

#[derive(Debug, Serialize)]
pub struct ModelListResponse {
    pub object: &'static str,
    pub data: Vec<ModelObject>,
}

#[derive(Debug, Serialize)]
pub struct ModelObject {
    pub id: String,
    pub object: &'static str,
    pub created: u64,
    pub owned_by: &'static str,
}

fn model_object(id: &str) -> ModelObject {
    ModelObject {
        id: id.to_string(),
        object: "model",
        created: 0,
        owned_by: "chiroo",
    }
}

pub async fn list_models(_principal: ApiPrincipal) -> Json<ModelListResponse> {
    let data = list_scenarios()
        .iter()
        .map(|info| model_object(info.id))
        .collect();
    Json(ModelListResponse {
        object: "list",
        data,
    })
}

pub async fn get_model(
    _principal: ApiPrincipal,
    Path(model): Path<String>,
) -> Result<Json<ModelObject>, OpenAiApiError> {
    let _definition =
        scenario_by_name(&model).ok_or_else(|| OpenAiApiError::model_not_found(&model))?;
    Ok(Json(model_object(&model)))
}

#[cfg(test)]
mod tests {
    use axum::response::IntoResponse;
    use uuid::Uuid;

    use super::*;
    use crate::domain::models::user::UserId;

    fn principal() -> ApiPrincipal {
        ApiPrincipal {
            user_id: UserId(Uuid::new_v4()),
        }
    }

    #[tokio::test]
    async fn list_models_lists_all_scenarios() {
        let response = list_models(principal()).await;
        let value = serde_json::to_value(&response.0).expect("serialize");
        assert_eq!(value["object"], "list");
        let ids: Vec<&str> = value["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, vec!["simple", "deep_research"]);
        let first = &value["data"][0];
        assert_eq!(first["object"], "model");
        assert_eq!(first["created"], 0);
        assert_eq!(first["owned_by"], "chiroo");
    }

    #[tokio::test]
    async fn get_model_returns_known_model() {
        let response = get_model(principal(), Path("simple".to_string()))
            .await
            .expect("known model");
        assert_eq!(response.0.id, "simple");
        assert_eq!(response.0.owned_by, "chiroo");
    }

    #[tokio::test]
    async fn get_model_unknown_returns_model_not_found() {
        let error = get_model(principal(), Path("gpt-4o".to_string()))
            .await
            .unwrap_err();
        let response = error.into_response();
        assert_eq!(response.status(), 404);
    }
}
