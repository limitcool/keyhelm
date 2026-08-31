//! 密钥 CRUD / reveal / resolve / import 处理函数

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::api::auth::Authorized;
use crate::api::AppState;
use crate::crypto;
use crate::model::{CreateSecretRequest, ImportItem, ResolveRequest, UpdateSecretRequest};

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub project: Option<String>,
    pub service: Option<String>,
    pub q: Option<String>,
    pub tag: Option<String>,
    #[serde(default)]
    pub page: i64,
    #[serde(default = "default_page_size")]
    pub page_size: i64,
    /// 返回明文值（默认不返回；web UI 列表直接展示时用）
    #[serde(default)]
    pub reveal: Option<String>,
}

fn default_page_size() -> i64 {
    50
}

/// GET /healthz（免鉴权）
pub async fn healthz(State(state): State<AppState>) -> impl IntoResponse {
    let _ = &state;
    Json(json!({ "status": "ok", "service": "keyhelm" }))
}

/// GET /api/v1/secrets — 列表（默认仅元数据；?reveal=1 时返回明文）
pub async fn list(
    State(state): State<AppState>,
    auth: Authorized,
    Query(q): Query<ListQuery>,
) -> Response {
    if let Err(resp) = auth.require_scope("read") {
        return resp;
    }
    match state
        .db
        .list_secrets(
            q.project.as_deref(),
            q.service.as_deref(),
            q.q.as_deref(),
            q.tag.as_deref(),
            q.page,
            q.page_size,
        )
        .await
    {
        Ok((items, total)) => {
            let items_json: Vec<_> = items
                .iter()
                .map(|s| {
                    let mut meta = serde_json::to_value(s.to_meta()).unwrap_or_default();
                    if q.reveal.is_some() {
                        match crypto::decrypt(&state.master_key, &s.value_enc) {
                            Ok(v) => {
                                meta["value"] = json!(v);
                            }
                            Err(e) => {
                                tracing::warn!("list reveal {}: {e}", s.id);
                                meta["value"] = json!(null);
                            }
                        }
                    }
                    meta
                })
                .collect();
            Json(json!({
                "items": items_json,
                "total": total,
                "page": q.page,
                "page_size": q.page_size,
            }))
            .into_response()
        }
        Err(e) => {
            tracing::error!("list secrets: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response()
        }
    }
}

/// POST /api/v1/secrets — 创建
pub async fn create(
    State(state): State<AppState>,
    auth: Authorized,
    Json(req): Json<CreateSecretRequest>,
) -> Response {
    if let Err(resp) = auth.require_scope("write") {
        return resp;
    }
    if req.project.trim().is_empty() || req.key_name.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "project 和 key_name 必填").into_response();
    }
    let value_enc = match crypto::encrypt(&state.master_key, &req.value) {
        Ok(enc) => enc,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };
    match state.db.create_secret(&req, &value_enc).await {
        Ok(Some(s)) => {
            let _ = state
                .db
                .audit(&auth.0.name(), "create", Some(&s.id), "")
                .await;
            (StatusCode::CREATED, Json(json!({ "secret": s.to_meta() }))).into_response()
        }
        Ok(None) => (
            StatusCode::CONFLICT,
            "该 project/service/key_name 已存在，用 PUT 更新或 POST /import upsert",
        )
            .into_response(),
        Err(e) => {
            tracing::error!("create secret: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response()
        }
    }
}

/// GET /api/v1/secrets/{id} — 元数据
pub async fn get(
    State(state): State<AppState>,
    auth: Authorized,
    Path(id): Path<String>,
) -> Response {
    if let Err(resp) = auth.require_scope("read") {
        return resp;
    }
    match state.db.get_secret(&id).await {
        Ok(Some(s)) => Json(json!({ "secret": s.to_meta() })).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "not found").into_response(),
        Err(e) => {
            tracing::error!("get secret: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response()
        }
    }
}

/// GET /api/v1/secrets/{id}/value — 解密 reveal（审计）
pub async fn reveal(
    State(state): State<AppState>,
    auth: Authorized,
    Path(id): Path<String>,
) -> Response {
    if let Err(resp) = auth.require_scope("read") {
        return resp;
    }
    let s = match state.db.get_secret(&id).await {
        Ok(Some(s)) => s,
        Ok(None) => return (StatusCode::NOT_FOUND, "not found").into_response(),
        Err(e) => {
            tracing::error!("reveal get: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response();
        }
    };
    match crypto::decrypt(&state.master_key, &s.value_enc) {
        Ok(value) => {
            let _ = state
                .db
                .audit(&auth.0.name(), "reveal", Some(&s.id), "")
                .await;
            Json(json!({
                "secret": s.to_meta(),
                "value": value,
            }))
            .into_response()
        }
        Err(e) => {
            tracing::warn!("reveal {}: {e}", s.id);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "解密失败：主密钥错误或数据损坏",
            )
                .into_response()
        }
    }
}

/// PUT /api/v1/secrets/{id} — 更新
pub async fn update(
    State(state): State<AppState>,
    auth: Authorized,
    Path(id): Path<String>,
    Json(req): Json<UpdateSecretRequest>,
) -> Response {
    if let Err(resp) = auth.require_scope("write") {
        return resp;
    }
    // 若提供了 value，先加密
    let new_enc = match &req.value {
        Some(v) => match crypto::encrypt(&state.master_key, v) {
            Ok(enc) => Some(enc),
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
        None => None,
    };
    match state.db.update_secret(&id, &req, new_enc.as_deref()).await {
        Ok(true) => {
            let _ = state
                .db
                .audit(&auth.0.name(), "update", Some(&id), "")
                .await;
            match state.db.get_secret(&id).await {
                Ok(Some(s)) => Json(json!({ "secret": s.to_meta() })).into_response(),
                _ => (StatusCode::OK, "updated").into_response(),
            }
        }
        Ok(false) => (StatusCode::NOT_FOUND, "not found").into_response(),
        Err(e) => {
            tracing::error!("update secret: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response()
        }
    }
}

/// DELETE /api/v1/secrets/{id}
pub async fn delete(
    State(state): State<AppState>,
    auth: Authorized,
    Path(id): Path<String>,
) -> Response {
    if let Err(resp) = auth.require_scope("write") {
        return resp;
    }
    match state.db.delete_secret(&id).await {
        Ok(true) => {
            let _ = state
                .db
                .audit(&auth.0.name(), "delete", Some(&id), "")
                .await;
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => (StatusCode::NOT_FOUND, "not found").into_response(),
        Err(e) => {
            tracing::error!("delete secret: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response()
        }
    }
}

/// POST /api/v1/resolve — AI 批量取值
pub async fn resolve(
    State(state): State<AppState>,
    auth: Authorized,
    Json(req): Json<ResolveRequest>,
) -> Response {
    if let Err(resp) = auth.require_scope("read") {
        return resp;
    }
    let mut results = Vec::with_capacity(req.items.len());
    for item in &req.items {
        let found = state
            .db
            .get_secret_by_key(
                &item.project,
                item.service.as_deref().unwrap_or(""),
                &item.key_name,
            )
            .await;
        match found {
            Ok(Some(s)) => match crypto::decrypt(&state.master_key, &s.value_enc) {
                Ok(value) => {
                    let _ = state
                        .db
                        .audit(&auth.0.name(), "resolve", Some(&s.id), "")
                        .await;
                    results.push(json!({
                        "project": item.project,
                        "service": item.service,
                        "key_name": item.key_name,
                        "value": value,
                    }));
                }
                Err(_) => results.push(json!({
                    "project": item.project,
                    "service": item.service,
                    "key_name": item.key_name,
                    "error": "decrypt_failed",
                })),
            },
            Ok(None) => results.push(json!({
                "project": item.project,
                "service": item.service,
                "key_name": item.key_name,
                "error": "not_found",
            })),
            Err(e) => {
                tracing::error!("resolve: {e}");
                results.push(json!({
                    "project": item.project,
                    "service": item.service,
                    "key_name": item.key_name,
                    "error": "db_error",
                }));
            }
        }
    }
    Json(json!({ "results": results })).into_response()
}

/// GET /api/v1/values/{project}/{key_name} — 单键快捷取值
pub async fn resolve_single(
    State(state): State<AppState>,
    auth: Authorized,
    Path((project, key_name)): Path<(String, String)>,
) -> Response {
    if let Err(resp) = auth.require_scope("read") {
        return resp;
    }
    match state.db.get_secret_by_key(&project, "", &key_name).await {
        Ok(Some(s)) => match crypto::decrypt(&state.master_key, &s.value_enc) {
            Ok(value) => {
                let _ = state
                    .db
                    .audit(&auth.0.name(), "resolve", Some(&s.id), "")
                    .await;
                Json(json!({ "value": value, "key_name": key_name, "project": project }))
                    .into_response()
            }
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "解密失败").into_response(),
        },
        Ok(None) => (StatusCode::NOT_FOUND, "not found").into_response(),
        Err(e) => {
            tracing::error!("resolve_single: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response()
        }
    }
}

/// POST /api/v1/import — AI 批量写入（upsert）
pub async fn import(
    State(state): State<AppState>,
    auth: Authorized,
    Json(items): Json<Vec<ImportItem>>,
) -> Response {
    if let Err(resp) = auth.require_scope("write") {
        return resp;
    }
    let mut created = 0usize;
    let mut updated = 0usize;
    let mut errors: Vec<serde_json::Value> = Vec::new();

    for item in &items {
        if item.project.trim().is_empty() || item.key_name.trim().is_empty() {
            errors.push(json!({ "key_name": item.key_name, "error": "project/key_name 必填" }));
            continue;
        }
        let value_enc = match crypto::encrypt(&state.master_key, &item.value) {
            Ok(enc) => enc,
            Err(e) => {
                errors.push(json!({ "key_name": item.key_name, "error": format!("encrypt: {e}") }));
                continue;
            }
        };
        // 判断 upsert 还是 create
        let existed = state
            .db
            .get_secret_by_key(&item.project, &item.service, &item.key_name)
            .await
            .unwrap_or(None);
        let source = if item.source.is_empty() {
            format!("api:{}", auth.0.name())
        } else {
            item.source.clone()
        };
        let identity = if item.identity.is_empty() {
            String::new()
        } else {
            item.identity.clone()
        };
        match state
            .db
            .upsert_secret(
                &item.project,
                &item.service,
                &item.key_name,
                &value_enc,
                &item.description,
                &item.tags,
                &source,
            )
            .await
        {
            Ok(s) => {
                if !identity.is_empty() {
                    let _ = state
                        .db
                        .set_identity(&item.project, &item.service, &identity)
                        .await;
                }
                if existed.is_some() {
                    updated += 1;
                } else {
                    created += 1;
                }
                let _ = state
                    .db
                    .audit(&auth.0.name(), "import", Some(&s.id), "")
                    .await;
            }
            Err(e) => {
                errors.push(json!({ "key_name": item.key_name, "error": format!("db: {e}") }));
            }
        }
    }

    Json(json!({ "created": created, "updated": updated, "errors": errors })).into_response()
}

/// GET /api/v1/projects — 项目树
pub async fn projects(State(state): State<AppState>, auth: Authorized) -> Response {
    if let Err(resp) = auth.require_scope("read") {
        return resp;
    }
    match state.db.list_projects().await {
        Ok(tree) => Json(json!({ "projects": tree })).into_response(),
        Err(e) => {
            tracing::error!("projects: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response()
        }
    }
}

/// PUT /api/v1/projects/{project}/icon — 设置项目自定义 lucide 图标
#[derive(Debug, Deserialize)]
pub struct SetIconRequest {
    #[serde(default)]
    pub icon: String,
}

pub async fn set_project_icon(
    State(state): State<AppState>,
    auth: Authorized,
    Path(project): Path<String>,
    Json(req): Json<SetIconRequest>,
) -> Response {
    if let Err(resp) = auth.require_scope("write") {
        return resp;
    }
    // 限制图标名格式：仅字母数字/连字符，避免注入 meta 键或前端
    let icon = req.icon.trim();
    if !icon.is_empty() && !icon.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return (StatusCode::BAD_REQUEST, "icon 只允许字母/数字/连字符").into_response();
    }
    match state.db.set_project_icon(&project, icon).await {
        Ok(()) => {
            let _ = state
                .db
                .audit(&auth.0.name(), &format!("set_icon:{project}"), None, "")
                .await;
            Json(json!({ "project": project, "icon": icon })).into_response()
        }
        Err(e) => {
            tracing::error!("set_project_icon: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response()
        }
    }
}
