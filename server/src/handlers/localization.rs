use axum::extract::State;
use axum::Json;
use serde_json::Value;
use std::collections::HashMap;

use crate::AppState;

pub async fn get_localizations(State(state): State<AppState>) -> Json<HashMap<String, Value>> {
    Json(state.localizations)
}
