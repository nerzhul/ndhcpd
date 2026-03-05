use crate::{models::Lease, AppState};
use axum::{extract::State, http::StatusCode, Json};
use tracing::error;

/// List all active leases
#[utoipa::path(
    get,
    path = "/api/leases",
    tag = "leases",
    responses(
        (status = 200, description = "List of active leases", body = Vec<Lease>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_leases(State(state): State<AppState>) -> Result<Json<Vec<Lease>>, StatusCode> {
    state
        .db
        .list_active_leases()
        .await
        .map(Json)
        .map_err(|e| {
            error!("Failed to list leases: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}
