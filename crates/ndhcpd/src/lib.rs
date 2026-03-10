pub mod auth;
pub mod config;
pub mod db;
pub mod dhcp;
pub mod handlers;
pub mod models;
pub mod routes;

pub use config::{Config, RaConfig};
pub use db::{create_database, Database, DynDatabase, InMemoryDatabase, SqliteDatabase};
pub use models::{DynamicRange, IAPrefix, StaticIP, Subnet};

use axum::{
    middleware,
    routing::{delete, get, patch, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::subnets::list_subnets,
        handlers::subnets::create_subnet,
        handlers::subnets::get_subnet,
        handlers::subnets::update_subnet,
        handlers::subnets::delete_subnet,
        handlers::ranges::list_ranges,
        handlers::ranges::create_range,
        handlers::ranges::delete_range,
        handlers::static_ips::list_static_ips,
        handlers::static_ips::create_static_ip,
        handlers::static_ips::delete_static_ip,
        handlers::leases::list_leases,
        handlers::tokens::list_tokens,
        handlers::tokens::create_token,
        handlers::tokens::delete_token,
        handlers::tokens::toggle_token,
        handlers::ia_prefixes::list_ia_prefixes,
        handlers::ia_prefixes::create_ia_prefix,
        handlers::ia_prefixes::get_ia_prefix,
        handlers::ia_prefixes::update_ia_prefix,
        handlers::ia_prefixes::delete_ia_prefix,
    ),
    components(
        schemas(
            models::Subnet,
            models::DynamicRange,
            models::StaticIP,
            models::Lease,
            models::ApiToken,
            models::CreateTokenRequest,
            models::CreateTokenResponse,
            models::IAPrefix,
        )
    ),
    tags(
        (name = "subnets", description = "Subnet management endpoints"),
        (name = "ranges", description = "Dynamic range management endpoints"),
        (name = "static-ips", description = "Static IP management endpoints"),
        (name = "leases", description = "Lease information endpoints"),
        (name = "tokens", description = "API token management endpoints"),
        (name = "ia-prefixes", description = "IPv6 prefix (IA Prefix) management for Router Advertisement"),
    )
)]
pub struct ApiDoc;

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub db: DynDatabase,
    pub ra_config: Arc<RaConfig>,
}

impl AppState {
    pub fn new(db: DynDatabase, ra_config: Arc<RaConfig>) -> Self {
        Self { db, ra_config }
    }
}

pub fn create_router(db: DynDatabase, ra_config: Arc<RaConfig>) -> Router {
    create_router_with_auth(db, ra_config, false)
}

pub fn create_router_with_auth(
    db: DynDatabase,
    ra_config: Arc<RaConfig>,
    require_auth: bool,
) -> Router {
    let state = AppState::new(db.clone(), ra_config);

    let protected_routes = Router::new()
        // Subnet routes
        .route("/api/subnets", get(handlers::subnets::list_subnets))
        .route("/api/subnets", post(handlers::subnets::create_subnet))
        .route("/api/subnets/:id", get(handlers::subnets::get_subnet))
        .route("/api/subnets/:id", put(handlers::subnets::update_subnet))
        .route("/api/subnets/:id", delete(handlers::subnets::delete_subnet))
        // Dynamic range routes
        .route("/api/ranges", get(handlers::ranges::list_ranges))
        .route("/api/ranges", post(handlers::ranges::create_range))
        .route("/api/ranges/:id", delete(handlers::ranges::delete_range))
        // Static IP routes
        .route(
            "/api/static-ips",
            get(handlers::static_ips::list_static_ips),
        )
        .route(
            "/api/static-ips",
            post(handlers::static_ips::create_static_ip),
        )
        .route(
            "/api/static-ips/:id",
            delete(handlers::static_ips::delete_static_ip),
        )
        // Lease routes
        .route("/api/leases", get(handlers::leases::list_leases))
        // Token management routes
        .route("/api/tokens", get(handlers::tokens::list_tokens))
        .route("/api/tokens", post(handlers::tokens::create_token))
        .route("/api/tokens/:id", delete(handlers::tokens::delete_token))
        .route(
            "/api/tokens/:id/toggle",
            patch(handlers::tokens::toggle_token),
        )
        // IA Prefix routes (IPv6 for Router Advertisement)
        .route(
            "/api/ia-prefixes",
            get(handlers::ia_prefixes::list_ia_prefixes),
        )
        .route(
            "/api/ia-prefixes",
            post(handlers::ia_prefixes::create_ia_prefix),
        )
        .route(
            "/api/ia-prefixes/:id",
            get(handlers::ia_prefixes::get_ia_prefix),
        )
        .route(
            "/api/ia-prefixes/:id",
            put(handlers::ia_prefixes::update_ia_prefix),
        )
        .route(
            "/api/ia-prefixes/:id",
            delete(handlers::ia_prefixes::delete_ia_prefix),
        );

    // Apply authentication middleware only if required
    let protected_routes = if require_auth {
        protected_routes.layer(middleware::from_fn_with_state(
            db.clone(),
            auth::auth_middleware,
        ))
    } else {
        protected_routes
    };

    let app = Router::new()
        .merge(protected_routes)
        // Health check - always public
        .route("/health", get(handlers::health::health_check))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Merge with Swagger UI
    let app =
        app.merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()));

    app
}
