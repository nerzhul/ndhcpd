use crate::{db::is_unique_violation, models::StaticIP, AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use tracing::error;

#[derive(Deserialize)]
pub struct StaticIpQuery {
    subnet_id: Option<i64>,
}

/// List all static IP assignments
#[utoipa::path(
    get,
    path = "/api/static-ips",
    tag = "static-ips",
    params(
        ("subnet_id" = Option<i64>, Query, description = "Filter by subnet ID")
    ),
    responses(
        (status = 200, description = "List of static IPs", body = Vec<StaticIP>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_static_ips(
    State(state): State<AppState>,
    Query(query): Query<StaticIpQuery>,
) -> Result<Json<Vec<StaticIP>>, StatusCode> {
    state
        .db
        .list_static_ips(query.subnet_id)
        .await
        .map(Json)
        .map_err(|e| {
            error!(
                "Failed to list static IPs (subnet_id={:?}): {}",
                query.subnet_id, e
            );
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

/// Create a new static IP assignment
#[utoipa::path(
    post,
    path = "/api/static-ips",
    tag = "static-ips",
    request_body = StaticIP,
    responses(
        (status = 201, description = "Static IP created", body = i64),
        (status = 400, description = "Bad request"),
        (status = 409, description = "Static IP already exists (duplicate MAC or IP)"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn create_static_ip(
    State(state): State<AppState>,
    Json(static_ip): Json<StaticIP>,
) -> Result<(StatusCode, Json<i64>), StatusCode> {
    state
        .db
        .create_static_ip(&static_ip)
        .await
        .map(|id| (StatusCode::CREATED, Json(id)))
        .map_err(|e| {
            if is_unique_violation(&e) {
                return StatusCode::CONFLICT;
            }
            error!(
                "Failed to create static IP (subnet_id={}, mac={}, ip={}): {}",
                static_ip.subnet_id, static_ip.mac_address, static_ip.ip_address, e
            );
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

/// Delete a static IP assignment
#[utoipa::path(
    delete,
    path = "/api/static-ips/{id}",
    tag = "static-ips",
    params(
        ("id" = i64, Path, description = "Static IP ID")
    ),
    responses(
        (status = 204, description = "Static IP deleted"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn delete_static_ip(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    state
        .db
        .delete_static_ip(id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| {
            error!("Failed to delete static IP id={}: {}", id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}
