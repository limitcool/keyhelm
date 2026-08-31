//! Web UI：登录/登出 + 静态资源服务

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::api::auth::{sign_jwt, Claims};
use crate::api::AppState;
use crate::ui::UiAssets;

/// 登录请求体（表单/JSON）
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// 校验 admin 密码。password_hash 为空 → 拒绝（bootstrap 已生成）
async fn verify_admin(state: &AppState, username: &str, password: &str) -> bool {
    let cfg = &state.cfg.auth;
    if username != cfg.admin_username {
        return false;
    }
    let hash = if !cfg.admin_password_hash.is_empty() {
        Some(cfg.admin_password_hash.clone())
    } else {
        // 从 meta 读 bootstrap 生成的 hash
        match state.db.meta_get("admin_password_hash").await {
            Ok(Some(h)) => Some(h),
            _ => None,
        }
    };
    match hash {
        Some(h) => verify_argon2(&h, password),
        None => false,
    }
}

fn verify_argon2(hash: &str, password: &str) -> bool {
    use argon2::password_hash::{PasswordHash, PasswordVerifier};
    match PasswordHash::new(hash) {
        Ok(parsed) => argon2::Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

/// POST /ui/login — 登录，设置 JWT session cookie
pub async fn login(State(state): State<AppState>, Json(req): Json<LoginRequest>) -> Response {
    if !verify_admin(&state, &req.username, &req.password).await {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "用户名或密码错误" })),
        )
            .into_response();
    }
    let exp =
        chrono::Utc::now().timestamp() as usize + state.cfg.auth.session_ttl_secs.max(60) as usize;
    let claims = Claims {
        sub: format!("admin:{}", req.username),
        exp,
        scopes: vec!["read".into(), "write".into(), "admin".into()],
        key_name: "web-session".into(),
    };
    match sign_jwt(&state.jwt_secret, &claims) {
        Ok(token) => {
            let cookie = format!(
                "keyhelm_session={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
                state.cfg.auth.session_ttl_secs.max(60)
            );
            (
                StatusCode::OK,
                [(header::SET_COOKIE, cookie.as_str())],
                Json(json!({ "ok": true })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("login sign: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "login failed").into_response()
        }
    }
}

/// POST /ui/logout — 清除 cookie
pub async fn logout() -> Response {
    (
        StatusCode::OK,
        [(
            header::SET_COOKIE,
            "keyhelm_session=; Path=/; HttpOnly; Max-Age=0",
        )],
        Json(json!({ "ok": true })),
    )
        .into_response()
}

/// GET /ui/login — 渲染登录页（前端路由到页面）
pub async fn login_page() -> Response {
    // 由前端 SPA 处理；这里返回静态 index.html
    match UiAssets::index_html() {
        Ok(html) => ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "index.html missing").into_response(),
    }
}

/// 静态资源 fallback：/ui/ 或根路径
pub async fn static_fallback(uri: axum::http::Uri) -> Response {
    let path = uri.path();
    let rel = if path == "/" || path == "/ui" {
        "index.html"
    } else {
        // rust-embed 的路径是相对 src/ui/static/ 的，去掉 /ui/ 前缀与开头的 /
        path.trim_start_matches("/ui/").trim_start_matches('/')
    };
    match UiAssets::get(rel) {
        Some(asset) => {
            let content_type = infer_content_type(rel);
            let body = asset.data.into_owned();
            ([(header::CONTENT_TYPE, content_type)], body).into_response()
        }
        None => {
            // SPA fallback → index.html
            match UiAssets::index_html() {
                Ok(html) => {
                    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response()
                }
                Err(_) => (StatusCode::NOT_FOUND, "not found").into_response(),
            }
        }
    }
}

fn infer_content_type(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".js") {
        "application/javascript; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else {
        "application/octet-stream"
    }
}
