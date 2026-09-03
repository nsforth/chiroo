use axum::extract::{Path as AxumPath, State};
use axum::Json;
use serde::Serialize;

use crate::errors::AppError;
use crate::handlers::branding::serve_asset;
use crate::services::scenarios::list_scenarios;
use crate::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelResponse {
    pub id: &'static str,
    pub icon: &'static str,
    pub icon_url: Option<String>,
}

pub async fn list_models(State(state): State<AppState>) -> Json<Vec<ModelResponse>> {
    let mut models = Vec::with_capacity(list_scenarios().len());
    for info in list_scenarios() {
        let icon_url = state
            .scenario_icons
            .get(info.id)
            .map(|asset| format!("/api/scenarios/{}/icon?v={}", info.id, asset.version));
        models.push(ModelResponse {
            id: info.id,
            icon: info.icon,
            icon_url,
        });
    }
    Json(models)
}

pub async fn get_scenario_icon(
    State(state): State<AppState>,
    AxumPath(scenario_id): AxumPath<String>,
) -> Result<axum::response::Response, AppError> {
    let asset = state.scenario_icons.get(&scenario_id).ok_or_else(|| {
        AppError::NotFound(format!("Scenario icon {scenario_id} is not configured"))
    })?;

    Ok(serve_asset(asset))
}
