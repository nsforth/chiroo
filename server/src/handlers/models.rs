use std::path::Path;

use axum::extract::{Path as AxumPath, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::errors::AppError;
use crate::handlers::branding::{file_version, serve_asset_file};
use crate::services::scenarios::list_scenarios;
use crate::AppState;

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
    pub icon_url: Option<String>,
}

async fn scenario_icon_url(dir: &Path, scenario_id: &str, icon_file: &str) -> Option<String> {
    let path = dir.join(icon_file);
    let version = file_version(&path).await?;
    Some(format!("/api/scenarios/{scenario_id}/icon?v={version}"))
}

pub async fn list_models(
    State(state): State<AppState>,
    Query(query): Query<ListModelsQuery>,
) -> Json<Vec<ModelResponse>> {
    let lang = query.lang.as_deref().unwrap_or("en");
    let icons_dir = state.config.branding.scenario_icons_dir.as_deref();

    let mut models = Vec::with_capacity(list_scenarios().len());
    for info in list_scenarios() {
        let icon_url = match (icons_dir, info.icon_file) {
            (Some(dir), Some(icon_file)) => scenario_icon_url(dir, info.id, icon_file).await,
            _ => None,
        };
        models.push(ModelResponse {
            id: info.id,
            icon: info.icon,
            label: info.label.get(lang),
            description: info.description.get(lang),
            icon_url,
        });
    }
    Json(models)
}

pub async fn get_scenario_icon(
    State(state): State<AppState>,
    AxumPath(scenario_id): AxumPath<String>,
) -> Result<axum::response::Response, AppError> {
    let info = list_scenarios()
        .iter()
        .find(|info| info.id == scenario_id)
        .ok_or_else(|| AppError::NotFound(format!("Unknown scenario: {scenario_id}")))?;

    let Some(icon_file) = info.icon_file else {
        return Err(AppError::NotFound(format!(
            "Scenario {scenario_id} has no icon"
        )));
    };

    let Some(dir) = state.config.branding.scenario_icons_dir.as_deref() else {
        return Err(AppError::NotFound(format!(
            "Scenario icons are not configured"
        )));
    };

    serve_asset_file(
        &format!("scenario {scenario_id} icon"),
        &dir.join(icon_file),
    )
    .await
}
