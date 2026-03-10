use anyhow::{anyhow, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2, PasswordHash, PasswordVerifier,
};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{engine::general_purpose, Engine as _};
use tracing::warn;

use crate::db::DynDatabase;

const TOKEN_LENGTH: usize = 32;

/// Generate a new API token
pub fn generate_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let token_bytes: Vec<u8> = (0..TOKEN_LENGTH).map(|_| rng.gen()).collect();
    general_purpose::STANDARD.encode(&token_bytes)
}

/// Hash a token with Argon2
pub fn hash_token(token: &str) -> Result<(String, String)> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    let password_hash = argon2
        .hash_password(token.as_bytes(), &salt)
        .map_err(|e| anyhow!("Failed to hash token: {}", e))?;

    Ok((password_hash.to_string(), salt.to_string()))
}

/// Verify a token against a stored hash
pub fn verify_token(token: &str, hash: &str) -> Result<bool> {
    let parsed_hash =
        PasswordHash::new(hash).map_err(|e| anyhow!("Failed to parse hash: {}", e))?;

    Ok(Argon2::default()
        .verify_password(token.as_bytes(), &parsed_hash)
        .is_ok())
}

/// Store connection type in request extensions
#[derive(Clone)]
pub enum ConnectionType {
    UnixSocket,
    Tcp,
}

/// Middleware to check API authentication
pub async fn auth_middleware(
    State(db): State<DynDatabase>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, impl IntoResponse> {
    // Check if this is a Unix socket connection (already set by router)
    if let Some(conn_type) = request.extensions().get::<ConnectionType>() {
        if matches!(conn_type, ConnectionType::UnixSocket) {
            return Ok(next.run(request).await);
        }
    }

    // Check for Authorization header
    let auth_header = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "Missing Authorization header"))?;

    // Extract Bearer token
    let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            "Invalid Authorization header format. Expected: Bearer <token>",
        )
    })?;

    // Verify token against database
    let valid = verify_token_in_db(&db, token).await.map_err(|e| {
        warn!("Token verification error: {}", e);
        (StatusCode::UNAUTHORIZED, "Invalid token")
    })?;

    if !valid {
        return Err((StatusCode::UNAUTHORIZED, "Invalid or disabled token"));
    }

    Ok(next.run(request).await)
}

/// Verify a token exists in the database and is enabled
async fn verify_token_in_db(db: &DynDatabase, token: &str) -> Result<bool> {
    let tokens = db.list_tokens().await?;

    for (token_hash, enabled) in tokens {
        if verify_token(token, &token_hash)? {
            let _ = db.update_token_last_used(&token_hash).await;
            return Ok(enabled == 1);
        }
    }

    Ok(false)
}
