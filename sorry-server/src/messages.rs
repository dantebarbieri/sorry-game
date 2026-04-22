use serde::{Deserialize, Serialize};
use sorry_core::{PlayerAction, PlayerView};

/// Client → server WebSocket messages. Tagged by `"type"` and reject unknown
/// fields so protocol drift surfaces loudly in development.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum ClientMessage {
    ConfigureSlot { slot: usize, player_type: String },
    SetNumPlayers { num_players: usize },
    SetRules { rules: String },
    KickPlayer { slot: usize },
    PromoteHost { slot: usize },
    SetTurnTimer { secs: Option<u64> },
    StartGame,
    Action { action: PlayerAction },
    PlayAgain,
    ReturnToLobby,
    Ping,
}

/// Server → client WebSocket messages.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    RoomState {
        state: RoomLobbyState,
    },
    GameState {
        state: PlayerView,
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_deadline_secs: Option<u64>,
    },
    ActionApplied {
        player: usize,
        action: PlayerAction,
        state: PlayerView,
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_deadline_secs: Option<u64>,
    },
    BotAction {
        player: usize,
        action: PlayerAction,
        state: PlayerView,
        #[serde(skip_serializing_if = "Option::is_none")]
        turn_deadline_secs: Option<u64>,
    },
    TimeoutAction {
        player: usize,
        action: PlayerAction,
        state: PlayerView,
    },
    PlayerJoined {
        player_index: usize,
        name: String,
    },
    PlayerLeft {
        player_index: usize,
    },
    PlayerReconnected {
        player_index: usize,
    },
    Kicked {
        reason: String,
    },
    Error {
        code: String,
        message: String,
    },
    Pong,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum PlayerSlotType {
    Human,
    Bot { strategy: String },
    Empty,
}

#[derive(Debug, Clone, Serialize)]
pub struct LobbyPlayer {
    pub slot: usize,
    pub name: String,
    pub player_type: PlayerSlotType,
    pub connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disconnect_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoomLobbyState {
    pub room_code: String,
    pub phase: String,
    pub players: Vec<LobbyPlayer>,
    pub num_players: usize,
    pub rules: String,
    pub creator: usize,
    pub available_strategies: Vec<String>,
    pub available_rules: Vec<String>,
    pub idle_timeout_secs: Option<u64>,
    pub turn_timer_secs: Option<u64>,
    pub last_winners: Vec<usize>,
}

// REST DTOs

#[derive(Debug, Deserialize)]
pub struct CreateRoomRequest {
    pub player_name: String,
    pub num_players: usize,
    pub rules: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateRoomResponse {
    pub room_code: String,
    pub session_token: String,
    pub player_index: usize,
}

#[derive(Debug, Deserialize)]
pub struct JoinRoomRequest {
    pub player_name: String,
}

#[derive(Debug, Serialize)]
pub struct JoinRoomResponse {
    pub session_token: String,
    pub player_index: usize,
}

#[derive(Debug, Serialize)]
pub struct RoomInfoResponse {
    pub room_code: String,
    pub num_players: usize,
    pub rules: String,
    pub players_joined: usize,
    pub phase: String,
}

#[derive(Debug, Serialize)]
pub struct MetaResponse {
    pub available_rules: Vec<String>,
    pub available_strategies: Vec<String>,
}
