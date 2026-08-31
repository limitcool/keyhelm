//! 管理端点：API keys

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::api::auth::{generate_api_key_token, hash_token, Authorized};
use crate::api::AppState;

/// POST /api/v1/api-keys — 创建（返回明文 token 仅此一次）
#[derive(Debug, Deserialize)]
pub struct CreateKeyRequest {
    pub name: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

pub async fn create_key(
    State(state): State<AppState>,
    auth: Authorized,
    Json(req): Json<CreateKeyRequest>,
) -> Response {
    if let Err(resp) = auth.require_scope("admin") {
        return resp;
    }
    if req.name.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "name 必填").into_response();
    }
    let scopes = if req.scopes.is_empty() {
        vec!["read".to_string()]
    } else {
        req.scopes
    };
    let token = generate_api_key_token();
    let hash = hash_token(&token);
    match state.db.create_api_key(&req.name, &hash, &scopes).await {
        Ok(key) => Json(json!({
            "api_key": key,
            "token": token,   // 仅此一次返回明文
            "note": "请立即保存 token，之后无法再次查看",
        }))
        .into_response(),
        Err(e) => {
            tracing::error!("create api key: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response()
        }
    }
}

/// GET /api/v1/api-keys — 列出（不含明文）
pub async fn list_keys(State(state): State<AppState>, auth: Authorized) -> Response {
    if let Err(resp) = auth.require_scope("admin") {
        return resp;
    }
    match state.db.list_api_keys().await {
        Ok(keys) => Json(json!({ "api_keys": keys })).into_response(),
        Err(e) => {
            tracing::error!("list api keys: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response()
        }
    }
}

/// DELETE /api/v1/api-keys/{id}
pub async fn delete_key(
    State(state): State<AppState>,
    auth: Authorized,
    Path(id): Path<String>,
) -> Response {
    if let Err(resp) = auth.require_scope("admin") {
        return resp;
    }
    match state.db.delete_api_key(&id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, "not found").into_response(),
        Err(e) => {
            tracing::error!("delete api key: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response()
        }
    }
}
