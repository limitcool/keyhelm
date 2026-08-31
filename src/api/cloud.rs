//! 云厂商集成端点：验证 key 有效性 + 探测资源

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::api::auth::Authorized;
use crate::api::AppState;
use crate::cloud;

/// 从探测/验证结果里提取「账号身份」用于持久化展示（非机密，如 RAM 用户 / 腾讯 AccountId / GCP email）
fn extract_identity(provider: &str, v: &serde_json::Value) -> String {
    let pick = |k: &[&str]| {
        v.get(k[0])
            .or_else(|| v.get(k[1]))
            .or_else(|| v.get(k[2]))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string()
    };
    match provider {
        "aliyun" => {
            // 优先 RAM 用户名（形如 user/hermes-xxx），其次 AccountId
            let user = v.get("user").and_then(|u| u.as_str()).unwrap_or("");
            if !user.is_empty() {
                user.to_string()
            } else {
                pick(&["account_id", "account", ""])
            }
        }
        "tencent" => pick(&["account_id", "account", ""]),
        "google-cloud" => pick(&["service_account", "email", "account"]),
        "cloudflare" => pick(&["email", "account", ""]),
        _ => String::new(),
    }
}

#[derive(Debug, Deserialize)]
pub struct CloudQuery {
    /// 该云厂商对应的 project（默认与 provider 同名）
    pub project: Option<String>,
}

/// POST /api/v1/cloud/{provider}/verify — 验证 key 有效性（同时写回账号身份）
pub async fn verify(
    State(state): State<AppState>,
    auth: Authorized,
    Path(provider): Path<String>,
    Query(q): Query<CloudQuery>,
) -> Response {
    if let Err(resp) = auth.require_scope("read") {
        return resp;
    }
    let project = q.project.as_deref().unwrap_or(&provider);
    match cloud::load_cloud_keys(&state.db, &state.master_key, project).await {
        Ok(keys) => match cloud::verify(&provider, &keys).await {
            Ok(info) => {
                let identity = extract_identity(&provider, &info);
                if !identity.is_empty() {
                    let _ = state.db.set_identity(project, "", &identity).await;
                }
                Json(info).into_response()
            }
            Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
        },
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/v1/cloud/{provider}/probe — 探测可访问资源
pub async fn probe(
    State(state): State<AppState>,
    auth: Authorized,
    Path(provider): Path<String>,
    Query(q): Query<CloudQuery>,
) -> Response {
    if let Err(resp) = auth.require_scope("read") {
        return resp;
    }
    let project = q.project.as_deref().unwrap_or(&provider);
    match cloud::load_cloud_keys(&state.db, &state.master_key, project).await {
        Ok(keys) => match cloud::probe(&provider, &keys).await {
            Ok(info) => {
                let identity = extract_identity(&provider, &info);
                if !identity.is_empty() {
                    let _ = state.db.set_identity(project, "", &identity).await;
                }
                // 探针结果（非机密资源/权限清单）持久化，卡片直接渲染
                let _ = state.db.set_probe_data(project, &info).await;
                Json(info).into_response()
            }
            Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
        },
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
