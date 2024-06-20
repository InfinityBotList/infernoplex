use std::{str::FromStr, sync::Arc};

use crate::shadowclaw::invite::{CreateInviteForUserError, CreateInviteForUserResult};
use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderMap, HeaderName};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    routing::post,
    Json, Router,
};
use botox::cache::CacheHttpImpl;
use log::info;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use strum::VariantNames;
use strum_macros::Display;
use tower_http::cors::{Any, CorsLayer};
use ts_rs::TS;
use utoipa::ToSchema;

#[allow(dead_code)]
pub struct AppState {
    pub cache_http: CacheHttpImpl,
    pub pool: PgPool,
    pub intents: serenity::all::GatewayIntents,
}

pub async fn setup_server(
    pool: PgPool,
    cache_http: CacheHttpImpl,
    intents: serenity::all::GatewayIntents,
) {
    use utoipa::OpenApi;
    #[derive(OpenApi)]
    #[openapi(
        paths(query),
        components(schemas(
            InfernoplexQuery,
            InfernoplexResponse,
            InfernoplexError,
            CreateInviteForUserResult,
            CreateInviteForUserError
        ))
    )]
    struct ApiDoc;

    async fn docs() -> impl IntoResponse {
        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", "application/json".parse().unwrap());
        let data = ApiDoc::openapi().to_json();

        if let Ok(data) = data {
            return (headers, data).into_response();
        }

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to generate docs".to_string(),
        )
            .into_response()
    }

    let shared_state = Arc::new(AppState {
        pool,
        cache_http,
        intents,
    });

    let app = Router::new()
        .route("/openapi", get(docs))
        .route("/", post(query))
        .with_state(shared_state)
        .layer(DefaultBodyLimit::max(1048576000))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
                .expose_headers([HeaderName::from_str("X-Session-Invalid").unwrap()]),
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

#[derive(Serialize, Deserialize, ToSchema, TS, Display, Clone, VariantNames)]
#[ts(export, export_to = ".generated/InfernoplexQuery.ts")]
pub enum InfernoplexQuery {
    /// Creates a new invite for a server
    ///
    /// If authentication is available, then a session token (for a user) may
    /// be optionally passed for login.
    ///
    /// This returns a ``Result<CreateInviteForUserResult, CreateInviteForUserError>``
    CreateInvite {
        session: Option<String>,
        guild_id: String,
    },
}

#[derive(Serialize, Deserialize, ToSchema, TS, Display, Clone, VariantNames)]
#[ts(export, export_to = ".generated/InfernoplexResponse.ts")]
pub enum InfernoplexResponse {
    /// The result of calling CreateInvite
    CreateInvite {
        /// Successfully created an invite
        result: CreateInviteForUserResult,
    },
}

impl IntoResponse for InfernoplexResponse {
    fn into_response(self) -> Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

#[derive(Serialize, Deserialize, ToSchema, TS, Display, Clone, VariantNames)]
#[ts(export, export_to = ".generated/InfernoplexError.ts")]
pub enum InfernoplexError {
    /// The result of calling CreateInvite
    CreateInvite {
        /// Successfully created an invite
        err: CreateInviteForUserError,
        /// Error message
        message: String,
    },
    /// A generic error message
    GenericError { message: String },
}

#[derive(Clone)]
pub struct InfernoplexErrorResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub error: InfernoplexError,
}

impl InfernoplexErrorResponse {
    pub fn new(status: StatusCode, headers: HeaderMap, error: InfernoplexError) -> Self {
        Self {
            status,
            headers,
            error,
        }
    }
}

impl IntoResponse for InfernoplexErrorResponse {
    fn into_response(self) -> Response {
        (self.status, self.headers, Json(self.error)).into_response()
    }
}

/// Make Infernoplex Query
#[utoipa::path(
    post,
    request_body = InfernoplexQuery,
    path = "/",
    responses(
        (status = 200, description = "The response of the query", body = InfernoplexResponse),
        (status = BAD_REQUEST, description = "An error occured performing the requested action", body = InfernoplexError),
    ),
)]
#[axum::debug_handler]
async fn query(
    State(state): State<Arc<AppState>>,
    Json(req): Json<InfernoplexQuery>,
) -> Result<InfernoplexResponse, InfernoplexErrorResponse> {
    match req {
        InfernoplexQuery::CreateInvite { guild_id, session } => {
            let guild_id = guild_id.parse::<serenity::all::GuildId>().map_err(|e| {
                InfernoplexErrorResponse::new(
                    StatusCode::BAD_REQUEST,
                    HeaderMap::new(),
                    InfernoplexError::GenericError {
                        message: format!("Invalid guild ID: {}", e),
                    },
                )
            })?;

            let user_id = if let Some(session) = session {
                let auth_session = super::auth::Session::from_token(&state.pool, &session)
                    .await
                    .map_err(|e| {
                        InfernoplexErrorResponse::new(
                            StatusCode::BAD_REQUEST,
                            HeaderMap::new(),
                            InfernoplexError::GenericError {
                                message: format!("Invalid session: {}", e),
                            },
                        )
                    })?;

                if let Some(auth_session) = auth_session {
                    // Check that target_type == 'user'
                    if auth_session.target_type != "user" {
                        return Err(InfernoplexErrorResponse::new(
                            StatusCode::FORBIDDEN,
                            HeaderMap::new(),
                            InfernoplexError::GenericError {
                                message: "CreateInvite can only be called on a user session"
                                    .to_string(),
                            },
                        ));
                    }

                    let user_id = auth_session
                        .target_id
                        .parse::<serenity::all::UserId>()
                        .map_err(|e| {
                            InfernoplexErrorResponse::new(
                                StatusCode::FORBIDDEN,
                                HeaderMap::new(),
                                InfernoplexError::GenericError {
                                    message: format!("Invalid user ID: {}", e),
                                },
                            )
                        })?;

                    Some(user_id)
                } else {
                    let mut headers = HeaderMap::new();
                    headers.insert("X-Session-Invalid", "1".parse().unwrap());
                    return Err(InfernoplexErrorResponse::new(
                        StatusCode::FORBIDDEN,
                        headers,
                        InfernoplexError::GenericError {
                            message: "Invalid session token".to_string(),
                        },
                    ));
                }
            } else {
                None
            };

            let created_invite = crate::shadowclaw::invite::create_invite_for_user(
                &state.cache_http,
                &state.pool,
                guild_id,
                user_id,
                false,
            )
            .await;

            log::info!("Created invite: {:?}", created_invite);

            match created_invite {
                Ok(invite) => Ok(InfernoplexResponse::CreateInvite { result: invite }),
                Err(e) => Err(InfernoplexErrorResponse::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    HeaderMap::new(),
                    InfernoplexError::CreateInvite {
                        err: e.clone(),
                        message: e.to_string(),
                    },
                )),
            }
        }
    }
}
