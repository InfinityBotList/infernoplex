use std::{collections::HashMap, fmt::Display, str::FromStr, sync::Arc};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
    routing::get,
    routing::post,
    Json, Router,
};
use log::info;
use poise::serenity_prelude::{AddMember, GuildId};
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{types::uuid, PgPool};
use tower_http::cors::{Any, CorsLayer};
use ts_rs::TS;

use botox::cache::{member_on_guild, CacheHttpImpl};

struct Error {
    status: StatusCode,
    message: String,
}

impl Error {
    fn new(e: impl Display) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: e.to_string(),
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (self.status, self.message).into_response()
    }
}

pub struct AppState {
    pub cache_http: CacheHttpImpl,
    pub pool: PgPool,
}

pub async fn setup_server(pool: PgPool, cache_http: CacheHttpImpl) {
    let shared_state = Arc::new(AppState { pool, cache_http });

    let app = Router::new().with_state(shared_state).layer(
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any),
    );

    let addr = format!("127.0.0.1:{}", crate::config::CONFIG.server_port.get());
    info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to port");

    if let Err(e) = axum::serve(listener, app.into_make_service()).await {
        panic!("RPC server error: {}", e);
    }
}
