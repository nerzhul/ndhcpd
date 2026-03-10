use crate::{db::is_unique_violation, models::Subnet, AppState};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use tracing::error;

/// List all subnets
#[utoipa::path(
    get,
    path = "/api/subnets",
    tag = "subnets",
    responses(
        (status = 200, description = "List of subnets", body = Vec<Subnet>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_subnets(State(state): State<AppState>) -> Result<Json<Vec<Subnet>>, StatusCode> {
    state.db.list_subnets().await.map(Json).map_err(|e| {
        error!("Failed to list subnets: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

/// Create a new subnet
#[utoipa::path(
    post,
    path = "/api/subnets",
    tag = "subnets",
    request_body = Subnet,
    responses(
        (status = 201, description = "Subnet created", body = i64),
        (status = 400, description = "Bad request"),
        (status = 409, description = "Subnet already exists"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn create_subnet(
    State(state): State<AppState>,
    Json(subnet): Json<Subnet>,
) -> Result<(StatusCode, Json<i64>), StatusCode> {
    state
        .db
        .create_subnet(&subnet)
        .await
        .map(|id| (StatusCode::CREATED, Json(id)))
        .map_err(|e| {
            if is_unique_violation(&e) {
                return StatusCode::CONFLICT;
            }
            error!(
                "Failed to create subnet (network={}/{}, gateway={}): {}",
                subnet.network, subnet.netmask, subnet.gateway, e
            );
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

/// Get a subnet by ID
#[utoipa::path(
    get,
    path = "/api/subnets/{id}",
    tag = "subnets",
    params(
        ("id" = i64, Path, description = "Subnet ID")
    ),
    responses(
        (status = 200, description = "Subnet found", body = Subnet),
        (status = 404, description = "Subnet not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_subnet(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Subnet>, StatusCode> {
    state
        .db
        .get_subnet(id)
        .await
        .map_err(|e| {
            error!("Failed to get subnet id={}: {}", id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// Update a subnet
#[utoipa::path(
    put,
    path = "/api/subnets/{id}",
    tag = "subnets",
    params(
        ("id" = i64, Path, description = "Subnet ID")
    ),
    request_body = Subnet,
    responses(
        (status = 200, description = "Subnet updated"),
        (status = 404, description = "Subnet not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn update_subnet(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(subnet): Json<Subnet>,
) -> Result<StatusCode, StatusCode> {
    state
        .db
        .update_subnet(id, &subnet)
        .await
        .map(|_| StatusCode::OK)
        .map_err(|e| {
            error!("Failed to update subnet id={}: {}", id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

/// Delete a subnet
#[utoipa::path(
    delete,
    path = "/api/subnets/{id}",
    tag = "subnets",
    params(
        ("id" = i64, Path, description = "Subnet ID")
    ),
    responses(
        (status = 204, description = "Subnet deleted"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn delete_subnet(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    state
        .db
        .delete_subnet(id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| {
            error!("Failed to delete subnet id={}: {}", id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}
