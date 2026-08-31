//! 鉴权：JWT (HS256) 签发与校验 + 角色 (scopes) 中间件

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::Engine;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::api::AppState;
use crate::model::ApiKey;

/// JWT 标准 + 自定义 claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,         // API key id 或 "admin:<username>"
    pub exp: usize,          // 过期时间（unix seconds）
    pub scopes: Vec<String>, // read / write / admin
    #[serde(default)]
    pub key_name: String, // API key 名（便于审计）
}

/// 从 config 加载 JWT 签名密钥（env 优先，其次文件）
pub fn load_jwt_secret(secret_env: &str, secret_file: &std::path::Path) -> anyhow::Result<Vec<u8>> {
    if let Ok(raw) = std::env::var(secret_env) {
        if !raw.trim().is_empty() {
            return Ok(raw.trim().as_bytes().to_vec());
        }
    }
    if secret_file.exists() {
        return Ok(std::fs::read(secret_file)?);
    }
    // 兜底：生成随机密钥并提示持久化（生产应显式配置）
    let bytes: Vec<u8> = (0..32).map(|_| rand::random::<u8>()).collect();
    tracing::warn!(
        "JWT 签名密钥未配置，使用随机临时密钥（重启后所有 JWT 失效）。请设置 env {} 或文件 {}",
        secret_env,
        secret_file.display()
    );
    Ok(bytes)
}

/// 签发 JWT
pub fn sign_jwt(secret: &[u8], claims: &Claims) -> anyhow::Result<String> {
    let key = EncodingKey::from_secret(secret);
    jsonwebtoken::encode(&Header::default(), claims, &key)
        .map_err(|e| anyhow::anyhow!("JWT 签发失败: {e}"))
}

/// 校验 JWT，返回 claims
pub fn verify_jwt(secret: &[u8], token: &str) -> anyhow::Result<Claims> {
    let key = DecodingKey::from_secret(secret);
    let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    validation.validate_exp = true;
    jsonwebtoken::decode::<Claims>(token, &key, &validation)
        .map(|d| d.claims)
        .map_err(|e| anyhow::anyhow!("JWT 校验失败: {e}"))
}

/// 认证结果
#[derive(Debug, Clone)]
pub enum Principal {
    ApiKey {
        key: ApiKey,
    },
    Admin {
        username: String,
        scopes: Vec<String>,
    },
}

impl Principal {
    pub fn has_scope(&self, scope: &str) -> bool {
        match self {
            Principal::ApiKey { key } => key.scopes.iter().any(|s| s == scope),
            Principal::Admin { scopes, .. } => scopes.iter().any(|s| s == scope),
        }
    }
    pub fn name(&self) -> String {
        match self {
            Principal::ApiKey { key } => format!("api:{}", key.name),
            Principal::Admin { username, .. } => format!("admin:{}", username),
        }
    }
}

/// 认证错误
pub struct AuthError(pub &'static str);

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        (StatusCode::UNAUTHORIZED, Json(json!({ "error": self.0 }))).into_response()
    }
}

/// 从请求中提取 Bearer JWT 并校验，得到 Principal
pub async fn authenticate(
    state: &AppState,
    headers: &axum::http::HeaderMap,
) -> Result<Principal, AuthError> {
    // 1. Bearer / X-API-Key 里的 JWT
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ").map(|s| s.to_string()))
        .or_else(|| {
            headers
                .get("X-API-Key")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        });

    if let Some(t) = token {
        return match verify_jwt(&state.jwt_secret, &t) {
            Ok(claims) => {
                if claims.sub.starts_with("admin:") {
                    Ok(Principal::Admin {
                        username: claims.sub.trim_start_matches("admin:").to_string(),
                        scopes: claims.scopes,
                    })
                } else {
                    // API key id
                    match state.db.get_api_key(&claims.sub).await {
                        Ok(Some(key)) => {
                            let _ = state.db.touch_api_key(&key.id).await;
                            Ok(Principal::ApiKey { key })
                        }
                        Ok(None) => Err(AuthError("JWT 对应的 API key 已被吊销")),
                        Err(_) => Err(AuthError("数据库错误")),
                    }
                }
            }
            Err(_) => Err(AuthError("JWT 无效或已过期")),
        };
    }

    // 2. Session cookie（Web UI）
    if let Some(cookie) = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()) {
        if let Some(token) = extract_cookie(cookie, "keyhelm_session") {
            if let Ok(claims) = verify_jwt(&state.jwt_secret, token) {
                if claims.sub.starts_with("admin:") {
                    return Ok(Principal::Admin {
                        username: claims.sub.trim_start_matches("admin:").to_string(),
                        scopes: claims.scopes,
                    });
                }
            }
        }
    }

    Err(AuthError("未认证：请提供 Bearer JWT 或登录"))
}

fn extract_cookie<'a>(cookie_header: &'a str, name: &str) -> Option<&'a str> {
    cookie_header.split(';').find_map(|part| {
        let part = part.trim();
        let needle = format!("{name}=");
        part.strip_prefix(&needle)
    })
}

/// 鉴权中间件：认证成功则将 Principal 注入 request extensions，否则返回 401
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    match authenticate(&state, req.headers()).await {
        Ok(principal) => {
            req.extensions_mut().insert(principal);
            next.run(req).await
        }
        Err(e) => e.into_response(),
    }
}

/// 从 request extensions 取已认证的 Principal（配合 auth_middleware 使用）
#[derive(Debug, Clone)]
pub struct Authorized(pub Principal);

impl Authorized {
    /// 校验某个 scope，失败返回 403
    pub fn require_scope(&self, scope: &str) -> Result<(), Response> {
        if self.0.has_scope(scope) {
            Ok(())
        } else {
            Err((StatusCode::FORBIDDEN, Json(json!({ "error": "权限不足" }))).into_response())
        }
    }
}

impl axum::extract::FromRequestParts<AppState> for Authorized {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Principal>()
            .cloned()
            .map(Authorized)
            .ok_or(AuthError("未认证"))
    }
}

/// 生成 API 访问 token（明文一次性返回）：keyhelm_<random>
pub fn generate_api_key_token() -> String {
    let mut bytes = [0u8; 24];
    rand::rng().fill_bytes(&mut bytes);
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    format!("kh_{b64}")
}

/// SHA-256 hash 一个 token 用于存储
pub fn hash_token(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 换取 token 的请求体
#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    pub api_key: String,
}

/// POST /api/v1/token — 用 API key 明文换取 JWT
pub async fn token_endpoint(
    State(state): State<AppState>,
    Json(req): Json<TokenRequest>,
) -> Response {
    let hash = hash_token(&req.api_key);
    let key = match state.db.get_api_key_by_hash(&hash).await {
        Ok(Some(k)) => k,
        Ok(None) => return AuthError("API key 无效").into_response(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "db error").into_response(),
    };
    let _ = state.db.touch_api_key(&key.id).await;
    let exp =
        chrono::Utc::now().timestamp() as usize + state.cfg.auth.session_ttl_secs.max(60) as usize;
    let claims = Claims {
        sub: key.id.clone(),
        exp,
        scopes: key.scopes.clone(),
        key_name: key.name.clone(),
    };
    match sign_jwt(&state.jwt_secret, &claims) {
        Ok(token) => Json(json!({
            "access_token": token,
            "token_type": "Bearer",
            "expires_in": state.cfg.auth.session_ttl_secs.max(60),
        }))
        .into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
