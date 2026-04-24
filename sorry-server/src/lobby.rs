use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use rand::Rng;
use tokio::sync::Mutex;

use crate::messages::{PlayerSlotType, ServerMessage};
use crate::room::{BroadcastTarget, Room, RoomPhase, SharedRoom};
use crate::session::SessionToken;

const ROOM_CODE_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const ROOM_CODE_LEN: usize = 6;
const LOBBY_IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Who a session token authenticates as inside a room.
#[derive(Debug, Clone)]
pub enum SessionRef {
    Player { code: String, slot: usize },
    Spectator { code: String },
}

impl SessionRef {
    pub fn code(&self) -> &str {
        match self {
            SessionRef::Player { code, .. } => code,
            SessionRef::Spectator { code } => code,
        }
    }
}

pub struct Lobby {
    pub rooms: DashMap<String, SharedRoom>,
    pub sessions: DashMap<String, SessionRef>,
    pub max_rooms: usize,
}

impl Lobby {
    pub fn new(max_rooms: usize) -> Self {
        Self {
            rooms: DashMap::new(),
            sessions: DashMap::new(),
            max_rooms,
        }
    }

    fn generate_code(&self) -> String {
        let mut rng = rand::rng();
        loop {
            let code: String = (0..ROOM_CODE_LEN)
                .map(|_| ROOM_CODE_ALPHABET[rng.random_range(0..ROOM_CODE_ALPHABET.len())] as char)
                .collect();
            if !self.rooms.contains_key(&code) {
                return code;
            }
        }
    }

    /// Returns `(room_code, session_token, player_index)` on success.
    pub fn create_room(
        &self,
        player_name: String,
        num_players: usize,
        rules: Option<String>,
    ) -> Result<(String, SessionToken, usize), String> {
        if !(2..=4).contains(&num_players) {
            return Err("Player count must be 2-4".to_string());
        }
        if self.rooms.len() >= self.max_rooms {
            return Err("Server is at maximum room capacity".to_string());
        }

        let code = self.generate_code();
        let token = SessionToken::new();
        let player_index = 0;

        let mut room = Room::new(code.clone(), player_name, num_players, rules);
        room.players[player_index].session_token = Some(token.clone());

        let shared = Arc::new(Mutex::new(room));
        self.rooms.insert(code.clone(), shared);
        self.sessions.insert(
            token.to_string(),
            SessionRef::Player {
                code: code.clone(),
                slot: player_index,
            },
        );

        Ok((code, token, player_index))
    }

    pub async fn join_room(
        &self,
        code: &str,
        player_name: String,
    ) -> Result<(SessionToken, usize), String> {
        let room_ref = self.rooms.get(code).ok_or("Room not found")?.clone();
        let mut room = room_ref.lock().await;

        if room.phase != RoomPhase::Lobby {
            return Err("Game already started".to_string());
        }

        let slot = room
            .next_available_slot()
            .ok_or("Room is full (all slots are taken by human players)")?;

        let token = SessionToken::new();
        room.players[slot].name = player_name.clone();
        room.players[slot].slot_type = PlayerSlotType::Human;
        room.players[slot].session_token = Some(token.clone());
        room.players[slot].connected = false;
        room.players[slot].disconnected_at = None;
        room.touch();

        self.sessions.insert(
            token.to_string(),
            SessionRef::Player {
                code: code.to_string(),
                slot,
            },
        );

        for (i, p) in room.players.iter().enumerate() {
            if i != slot && p.connected {
                let _ = room.broadcast_tx.send((
                    BroadcastTarget::Player(i),
                    ServerMessage::PlayerJoined {
                        player_index: slot,
                        name: player_name.clone(),
                    },
                ));
            }
        }

        Ok((token, slot))
    }

    /// Join a room as a spectator. No phase restriction — spectators can
    /// join in lobby, mid-game, or post-game. Returns the session token
    /// and the newly appended spectator index.
    pub async fn spectate_room(
        &self,
        code: &str,
        player_name: String,
    ) -> Result<(SessionToken, usize), String> {
        let room_ref = self.rooms.get(code).ok_or("Room not found")?.clone();
        let mut room = room_ref.lock().await;

        let token = SessionToken::new();
        let idx = room.add_spectator(player_name, token.clone());

        self.sessions.insert(
            token.to_string(),
            SessionRef::Spectator {
                code: code.to_string(),
            },
        );

        Ok((token, idx))
    }

    pub fn get_session(&self, token: &str) -> Option<SessionRef> {
        self.sessions.get(token).map(|entry| entry.clone())
    }

    pub fn get_room(&self, code: &str) -> Option<SharedRoom> {
        self.rooms.get(code).map(|entry| entry.clone())
    }

    /// Remove rooms that have been idle past the configured thresholds.
    /// Uses `try_lock` to skip rooms in active use — we'd rather reap next
    /// pass than block the cleanup task on an in-flight handler.
    pub fn cleanup_stale_rooms(&self, game_over_timeout: Duration, disconnect_timeout: Duration) {
        let now = Instant::now();
        let mut to_remove = Vec::new();

        for entry in self.rooms.iter() {
            let code = entry.key().clone();
            if let Ok(room) = entry.value().try_lock() {
                let elapsed = now.duration_since(room.last_activity);
                let all_disconnected = room.players.iter().all(|p| !p.connected);
                let should_remove = match room.phase {
                    RoomPhase::GameOver => elapsed > game_over_timeout,
                    RoomPhase::Lobby => {
                        (all_disconnected && elapsed > disconnect_timeout)
                            || elapsed > LOBBY_IDLE_TIMEOUT
                    }
                    RoomPhase::InGame => all_disconnected && elapsed > disconnect_timeout,
                };
                if should_remove {
                    for p in &room.players {
                        if let Some(token) = &p.session_token {
                            self.sessions.remove(token.as_str());
                        }
                    }
                    for s in &room.spectators {
                        self.sessions.remove(s.session_token.as_str());
                    }
                    to_remove.push(code);
                }
            }
        }

        for code in to_remove {
            self.rooms.remove(&code);
            tracing::info!("Cleaned up stale room: {code}");
        }
    }
}
