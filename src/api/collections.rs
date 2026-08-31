//! 分组管理端点（admin scope）

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use crate::api::auth::Authorized;
use crate::api::AppState;
use crate::model::CreateCollectionRequest;

/// POST /api/v1/collections — 创建分组
pub async fn create_collection(
    State(state): State<AppState>,
    auth: Authorized,
    Json(req): Json<CreateCollectionRequest>,
) -> Response {
    if let Err(resp) = auth.require_scope("admin") {
        return resp;
    }
    if req.name.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "name 必填").into_response();
    }
    match state
        .db
        .create_collection(&req.name, &req.description)
        .await
    {
        Ok(c) => (StatusCode::CREATED, Json(json!({ "collection": c }))).into_response(),
        Err(e) => {
            tracing::error!("create collection: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response()
        }
    }
}

/// GET /api/v1/collections — 列出
pub async fn list_collections(State(state): State<AppState>, auth: Authorized) -> Response {
    if let Err(resp) = auth.require_scope("read") {
        return resp;
    }
    match state.db.list_collections().await {
        Ok(cols) => Json(json!({ "collections": cols })).into_response(),
        Err(e) => {
            tracing::error!("list collections: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response()
        }
    }
}

/// DELETE /api/v1/collections/{id}
pub async fn delete_collection(
    State(state): State<AppState>,
    auth: Authorized,
    Path(id): Path<String>,
) -> Response {
    if let Err(resp) = auth.require_scope("admin") {
        return resp;
    }
    match state.db.delete_collection(&id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, "not found").into_response(),
        Err(e) => {
            tracing::error!("delete collection: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response()
        }
    }
}

/// GET /api/v1/collections/{id}/items — 列出分组内的密钥（元数据，不含值）
pub async fn list_items(
    State(state): State<AppState>,
    auth: Authorized,
    Path(id): Path<String>,
) -> Response {
    if let Err(resp) = auth.require_scope("read") {
        return resp;
    }
    match state.db.list_collection_items(&id).await {
        Ok(items) => Json(json!({ "secrets": items })).into_response(),
        Err(e) => {
            tracing::error!("list collection items: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response()
        }
    }
}

/// PUT /api/v1/collections/{id}/items/{secret_id} — 加入分组
pub async fn add_item(
    State(state): State<AppState>,
    auth: Authorized,
    Path((id, secret_id)): Path<(String, String)>,
) -> Response {
    if let Err(resp) = auth.require_scope("write") {
        return resp;
    }
    match state.db.add_item(&id, &secret_id).await {
        Ok(true) => (StatusCode::OK, Json(json!({ "ok": true }))).into_response(),
        Ok(false) => (StatusCode::CONFLICT, "已存在").into_response(),
        Err(e) => {
            tracing::error!("add item: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response()
        }
    }
}

/// DELETE /api/v1/collections/{id}/items/{secret_id} — 移出分组
pub async fn remove_item(
    State(state): State<AppState>,
    auth: Authorized,
    Path((id, secret_id)): Path<(String, String)>,
) -> Response {
    if let Err(resp) = auth.require_scope("write") {
        return resp;
    }
    match state.db.remove_item(&id, &secret_id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (StatusCode::NOT_FOUND, "not found").into_response(),
        Err(e) => {
            tracing::error!("remove item: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response()
        }
    }
}
