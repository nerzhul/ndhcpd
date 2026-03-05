use crate::{models::IAPrefix, AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use tracing::error;

/// Query parameters for listing prefixes
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    /// Filter by interface name
    pub interface: Option<String>,
}

/// List all IPv6 prefixes (optionally filtered by interface)
#[utoipa::path(
    get,
    path = "/api/ia-prefixes",
    tag = "ia-prefixes",
    params(
        ("interface" = Option<String>, Query, description = "Filter by interface name")
    ),
    responses(
        (status = 200, description = "List of IPv6 prefixes", body = Vec<IAPrefix>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_ia_prefixes(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<IAPrefix>>, StatusCode> {
    let interface = query.interface.as_deref();
    state
        .db
        .list_ia_prefixes(interface)
        .await
        .map(Json)
        .map_err(|e| {
            error!("Failed to list IA prefixes (interface={:?}): {}", interface, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

/// Create a new IPv6 prefix
#[utoipa::path(
    post,
    path = "/api/ia-prefixes",
    tag = "ia-prefixes",
    request_body = IAPrefix,
    responses(
        (status = 201, description = "IPv6 prefix created", body = i64),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn create_ia_prefix(
    State(state): State<AppState>,
    Json(mut prefix): Json<IAPrefix>,
) -> Result<(StatusCode, Json<i64>), StatusCode> {
    // Apply default values from config if not specified
    if prefix.preferred_lifetime == 0 {
        prefix.preferred_lifetime = state.ra_config.default_preferred_lifetime;
    }
    if prefix.valid_lifetime == 0 {
        prefix.valid_lifetime = state.ra_config.default_valid_lifetime;
    }
    if prefix.dns_lifetime == 0 {
        prefix.dns_lifetime = state.ra_config.default_dns_lifetime;
    }

    state
        .db
        .create_ia_prefix(&prefix)
        .await
        .map(|id| (StatusCode::CREATED, Json(id)))
        .map_err(|e| {
            error!("Failed to create IA prefix (interface={}, prefix={}): {}", prefix.interface, prefix.prefix, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

/// Get an IPv6 prefix by ID
#[utoipa::path(
    get,
    path = "/api/ia-prefixes/{id}",
    tag = "ia-prefixes",
    params(
        ("id" = i64, Path, description = "IPv6 prefix ID")
    ),
    responses(
        (status = 200, description = "IPv6 prefix found", body = IAPrefix),
        (status = 404, description = "IPv6 prefix not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_ia_prefix(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<IAPrefix>, StatusCode> {
    state
        .db
        .get_ia_prefix(id)
        .await
        .map_err(|e| {
            error!("Failed to get IA prefix id={}: {}", id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// Update an IPv6 prefix
#[utoipa::path(
    put,
    path = "/api/ia-prefixes/{id}",
    tag = "ia-prefixes",
    params(
        ("id" = i64, Path, description = "IPv6 prefix ID")
    ),
    request_body = IAPrefix,
    responses(
        (status = 200, description = "IPv6 prefix updated"),
        (status = 404, description = "IPv6 prefix not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn update_ia_prefix(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(prefix): Json<IAPrefix>,
) -> Result<StatusCode, StatusCode> {
    state
        .db
        .update_ia_prefix(id, &prefix)
        .await
        .map(|_| StatusCode::OK)
        .map_err(|e| {
            error!("Failed to update IA prefix id={}: {}", id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

/// Delete an IPv6 prefix
#[utoipa::path(
    delete,
    path = "/api/ia-prefixes/{id}",
    tag = "ia-prefixes",
    params(
        ("id" = i64, Path, description = "IPv6 prefix ID")
    ),
    responses(
        (status = 204, description = "IPv6 prefix deleted"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn delete_ia_prefix(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, StatusCode> {
    state
        .db
        .delete_ia_prefix(id)
        .await
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| {
            error!("Failed to delete IA prefix id={}: {}", id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}
