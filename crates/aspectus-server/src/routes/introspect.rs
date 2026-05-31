use axum::{extract::State, http::StatusCode, response::IntoResponse, Form, Json};
use serde::Deserialize;

use crate::AppState;

#[derive(Deserialize)]
pub struct IntrospectForm {
    token: String,
    #[serde(default)]
    token_type_hint: Option<String>,
}

pub async fn handle(
    State(state): State<AppState>,
    Form(form): Form<IntrospectForm>,
) -> impl IntoResponse {
    let response = state.api_key_verifier.verify(&form.token).await;
    (StatusCode::OK, Json(response))
}
