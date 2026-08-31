//! API 层：Axum 路由装配

pub mod admin;
pub mod auth;
pub mod cloud;
pub mod collections;
pub mod secrets;
pub mod web;

use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::middleware::from_fn_with_state;
use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::crypto::MasterKey;
use crate::db::Db;

/// 应用共享状态
#[derive(Clone)]
pub struct AppState {
    pub db: Db,
    pub master_key: MasterKey,
    pub jwt_secret: Arc<Vec<u8>>,
    pub cfg: Arc<Config>,
}

/// 构建完整路由（REST API + Web UI）
pub fn build_router(state: AppState) -> Router {
    // 受保护的路由（挂 auth 中间件）
    let protected = Router::new()
        .route("/secrets", get(secrets::list).post(secrets::create))
        .route(
            "/secrets/{id}",
            get(secrets::get)
                .put(secrets::update)
                .delete(secrets::delete),
        )
        .route("/secrets/{id}/value", get(secrets::reveal))
        .route("/resolve", post(secrets::resolve))
        .route("/values/{project}/{key_name}", get(secrets::resolve_single))
        .route("/import", post(secrets::import))
        .route("/projects", get(secrets::projects))
        .route(
            "/projects/{project}/icon",
            axum::routing::put(secrets::set_project_icon),
        )
        .route("/api-keys", get(admin::list_keys).post(admin::create_key))
        .route("/api-keys/{id}", axum::routing::delete(admin::delete_key))
        .route("/cloud/{provider}/verify", post(cloud::verify))
        .route("/cloud/{provider}/probe", post(cloud::probe))
        .route(
            "/collections",
            get(collections::list_collections).post(collections::create_collection),
        )
        .route(
            "/collections/{id}",
            axum::routing::delete(collections::delete_collection),
        )
        .route("/collections/{id}/items", get(collections::list_items))
        .route(
            "/collections/{id}/items/{secret_id}",
            axum::routing::put(collections::add_item).delete(collections::remove_item),
        );

    // 免鉴权的：healthz + token 换取
    let open = Router::new()
        .route("/healthz", get(secrets::healthz))
        .route("/token", post(auth::token_endpoint));

    let protected = protected.layer(from_fn_with_state(state.clone(), auth::auth_middleware));

    let api = Router::new()
        .merge(open)
        .merge(protected)
        .with_state(state.clone());

    let web = Router::new()
        .route("/ui/login", post(web::login).get(web::login_page))
        .route("/ui/logout", post(web::logout))
        .fallback(web::static_fallback)
        .with_state(state.clone());

    // CORS（可选）
    let cors = if state.cfg.server.cors_origins.is_empty() {
        CorsLayer::permissive()
    } else {
        let origins = state
            .cfg
            .server
            .cors_origins
            .iter()
            .filter_map(|o| o.parse::<axum::http::HeaderValue>().ok())
            .collect::<Vec<_>>();
        CorsLayer::new().allow_origin(origins)
    };

    Router::new()
        .nest("/api/v1", api)
        .merge(web)
        .layer(cors)
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
        .layer(TraceLayer::new_for_http())
}
