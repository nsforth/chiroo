use axum::extract::Query;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::services::scenarios::list_scenarios;

#[derive(Deserialize)]
pub struct ListModelsQuery {
    #[serde(default)]
    pub lang: Option<String>,
}

#[derive(Serialize)]
pub struct ModelResponse {
    pub id: &'static str,
    pub icon: &'static str,
    pub label: &'static str,
    pub description: &'static str,
}

pub async fn list_models(Query(query): Query<ListModelsQuery>) -> Json<Vec<ModelResponse>> {
    let lang = query.lang.as_deref().unwrap_or("en");
    let models = list_scenarios()
        .iter()
        .map(|info| ModelResponse {
            id: info.id,
            icon: info.icon,
            label: info.label.get(lang),
            description: info.description.get(lang),
        })
        .collect();
    Json(models)
}
