use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use tracing::error;

use crate::{
    auth::{generate_token, hash_token},
    db::is_unique_violation,
    models::{ApiToken, CreateTokenRequest, CreateTokenResponse},
    AppState,
};

/// List all API tokens
#[utoipa::path(
    get,
    path = "/api/tokens",
    responses(
        (status = 200, description = "List of API tokens", body = Vec<ApiToken>),
        (status = 500, description = "Internal server error")
    ),
    tag = "tokens"
)]
pub async fn list_tokens(
    State(state): State<AppState>,
) -> Result<Json<Vec<ApiToken>>, impl IntoResponse> {
    match state.db.list_api_tokens().await {
        Ok(tokens) => Ok(Json(tokens)),
        Err(e) => {
            error!("Failed to list tokens: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to list tokens"))
        }
    }
}

/// Create a new API token
#[utoipa::path(
    post,
    path = "/api/tokens",
    request_body = CreateTokenRequest,
    responses(
        (status = 201, description = "Token created successfully", body = CreateTokenResponse),
        (status = 400, description = "Bad request"),
        (status = 409, description = "Token name already exists"),
        (status = 500, description = "Internal server error")
    ),
    tag = "tokens"
)]
pub async fn create_token(
    State(state): State<AppState>,
    Json(request): Json<CreateTokenRequest>,
) -> Result<(StatusCode, Json<CreateTokenResponse>), impl IntoResponse> {
    let token = generate_token();
    let (token_hash, salt) = hash_token(&token).map_err(|e| {
        error!("Failed to hash token: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to hash token")
    })?;

    match state
        .db
        .create_token(&request.name, &token_hash, &salt)
        .await
    {
        Ok(id) => {
            let response = CreateTokenResponse {
                id,
                name: request.name,
                token,
            };
            Ok((StatusCode::CREATED, Json(response)))
        }
        Err(e) => {
            if is_unique_violation(&e) {
                return Err((StatusCode::CONFLICT, "Token name already exists"));
            }
            error!("Failed to create token: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to create token"))
        }
    }
}

/// Delete an API token
#[utoipa::path(
    delete,
    path = "/api/tokens/{id}",
    params(
        ("id" = i64, Path, description = "Token ID")
    ),
    responses(
        (status = 204, description = "Token deleted successfully"),
        (status = 500, description = "Internal server error")
    ),
    tag = "tokens"
)]
pub async fn delete_token(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, impl IntoResponse> {
    match state.db.delete_token(id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => {
            error!("Failed to delete token: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to delete token"))
        }
    }
}

/// Enable/disable an API token
#[utoipa::path(
    patch,
    path = "/api/tokens/{id}/toggle",
    params(
        ("id" = i64, Path, description = "Token ID")
    ),
    responses(
        (status = 200, description = "Token toggled successfully"),
        (status = 404, description = "Token not found"),
        (status = 500, description = "Internal server error")
    ),
    tag = "tokens"
)]
pub async fn toggle_token(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, impl IntoResponse> {
    // Get current state to determine the flip
    let tokens = state.db.list_api_tokens().await.map_err(|e| {
        error!("Failed to list tokens: {}", e);
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to toggle token")
    })?;

    let current = tokens.iter().find(|t| t.id == Some(id));
    match current {
        None => Err((StatusCode::NOT_FOUND, "Token not found")),
        Some(token) => match state.db.toggle_token(id, !token.enabled).await {
            Ok(_) => Ok(StatusCode::OK),
            Err(e) => {
                error!("Failed to toggle token: {}", e);
                Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to toggle token"))
            }
        },
    }
}
