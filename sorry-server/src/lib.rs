//! `sorry-server` — axum + WebSocket multiplayer host for the Sorry! engine.
//!
//! The binary in `main.rs` is a thin bootstrap; everything testable lives
//! here so integration tests can import and spin up a server in-process.

pub mod lobby;
pub mod messages;
pub mod room;
pub mod session;
pub mod strategies;
pub mod ws;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::extract::{ConnectInfo, Path, Query, State, WebSocketUpgrade};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::response::{Json, Response};
use axum::routing::{get, post};
use serde::Deserialize;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;

use lobby::Lobby;
use messages::{
    CreateRoomRequest, CreateRoomResponse, JoinRoomRequest, JoinRoomResponse, MetaResponse,
    PlayerSlotType, RoomInfoResponse,
};

pub struct AppStateInner {
    pub lobby: Lobby,
}

pub type AppState = Arc<AppStateInner>;

pub fn build_app(state: AppState, cors_origin: Option<HeaderValue>) -> Router {
    let api_routes: Router<AppState> = Router::new()
        .route("/rooms", post(create_room))
        .route("/rooms/{code}", get(room_info))
        .route("/rooms/{code}/join", post(join_room))
        .route("/rooms/{code}/ws", get(ws_upgrade))
        .route("/meta", get(meta));

    let mut app = Router::new()
        .nest("/api", api_routes)
        .layer(CompressionLayer::new());

    if let Some(origin) = cors_origin {
        let cors = CorsLayer::new()
            .allow_origin(origin)
            .allow_methods([Method::GET, Method::POST])
            .allow_headers([header::CONTENT_TYPE]);
        app = app.layer(cors);
    }

    app.with_state(state)
}

async fn create_room(
    State(state): State<AppState>,
    Json(req): Json<CreateRoomRequest>,
) -> Result<Json<CreateRoomResponse>, (StatusCode, String)> {
    let (code, token, player_index) = state
        .lobby
        .create_room(req.player_name, req.num_players, req.rules)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(Json(CreateRoomResponse {
        room_code: code,
        session_token: token.to_string(),
        player_index,
    }))
}

async fn room_info(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<RoomInfoResponse>, (StatusCode, String)> {
    let room_ref = state
        .lobby
        .get_room(&code)
        .ok_or((StatusCode::NOT_FOUND, "Room not found".to_string()))?;
    let room = room_ref.lock().await;
    let players_joined = room
        .players
        .iter()
        .filter(|p| p.slot_type != PlayerSlotType::Empty)
        .count();
    Ok(Json(RoomInfoResponse {
        room_code: room.code.clone(),
        num_players: room.num_players,
        rules: room.rules_name.clone(),
        players_joined,
        phase: room.phase_label().to_string(),
    }))
}

async fn join_room(
    State(state): State<AppState>,
    Path(code): Path<String>,
    Json(req): Json<JoinRoomRequest>,
) -> Result<Json<JoinRoomResponse>, (StatusCode, String)> {
    let (token, player_index) = state
        .lobby
        .join_room(&code, req.player_name)
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(Json(JoinRoomResponse {
        session_token: token.to_string(),
        player_index,
    }))
}

#[derive(Deserialize)]
struct WsQuery {
    token: String,
}

async fn ws_upgrade(
    State(state): State<AppState>,
    Path(code): Path<String>,
    Query(query): Query<WsQuery>,
    ConnectInfo(_addr): ConnectInfo<SocketAddr>,
    ws: WebSocketUpgrade,
) -> Result<Response, (StatusCode, String)> {
    let (room_code, player_index) = state
        .lobby
        .get_session(&query.token)
        .ok_or((StatusCode::UNAUTHORIZED, "Invalid session token".to_string()))?;

    if room_code != code {
        return Err((
            StatusCode::FORBIDDEN,
            "Token does not match this room".to_string(),
        ));
    }

    let room = state
        .lobby
        .get_room(&code)
        .ok_or((StatusCode::NOT_FOUND, "Room not found".to_string()))?;

    Ok(ws.on_upgrade(move |socket| async move {
        ws::handle_ws(socket, state, room, room_code, player_index).await;
    }))
}

async fn meta() -> Json<MetaResponse> {
    Json(MetaResponse {
        available_rules: strategies::available_rules(),
        available_strategies: strategies::available_strategies(),
    })
}
