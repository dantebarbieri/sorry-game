use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket};
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use tokio::sync::broadcast;

use crate::AppState;
use crate::lobby::SessionRef;
use crate::messages::{ClientMessage, ServerMessage};
use crate::room::{BroadcastTarget, RoomPhase, SharedRoom};

const DISCONNECT_AUTO_KICK_SECS: u64 = 60;
const SPECTATOR_AUTO_KICK_SECS: u64 = 300;
const HOST_AUTO_PROMOTE_SECS: u64 = 10;
const BOT_TURN_DELAY_MS: u64 = 500;

pub async fn handle_player_ws(
    ws: WebSocket,
    state: AppState,
    room: SharedRoom,
    room_code: String,
    player_index: usize,
) {
    // Mutable because the player may move between slots via `TakeSlot`.
    let mut player_index = player_index;
    let (mut ws_tx, mut ws_rx) = ws.split();

    let mut broadcast_rx = {
        let mut room_guard = room.lock().await;
        room_guard.players[player_index].connected = true;
        room_guard.players[player_index].disconnected_at = None;
        room_guard.touch();

        let initial = match room_guard.phase {
            RoomPhase::Lobby | RoomPhase::GameOver => ServerMessage::RoomState {
                state: room_guard.lobby_state(),
            },
            RoomPhase::InGame => match room_guard.player_view(player_index) {
                Some(state) => ServerMessage::GameState {
                    state,
                    turn_deadline_secs: room_guard.turn_deadline_secs(),
                },
                None => ServerMessage::RoomState {
                    state: room_guard.lobby_state(),
                },
            },
        };
        send_msg(&mut ws_tx, &initial).await;

        for (i, slot) in room_guard.players.iter().enumerate() {
            if i != player_index && slot.connected {
                let _ = room_guard.broadcast_tx.send((
                    BroadcastTarget::Player(i),
                    ServerMessage::PlayerReconnected { player_index },
                ));
            }
        }

        room_guard.broadcast_tx.subscribe()
    };

    // Tracks whether this socket is now servicing a spectator session
    // (because the client sent BecomeSpectator). When set, the outer loop
    // switches filtering to AllSpectators and stops accepting seated-only
    // messages for the remainder of the connection.
    let mut became_spectator: Option<String> = None;

    loop {
        tokio::select! {
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Some(spec_token) = &became_spectator {
                            let response = handle_spectator_message(
                                text.as_ref(),
                                &state,
                                &room,
                                spec_token,
                            ).await;
                            if let Some(msg) = response {
                                send_msg(&mut ws_tx, &msg).await;
                            }
                        } else {
                            let outcome = handle_player_message(
                                text.as_ref(),
                                &state,
                                &room,
                                player_index,
                            ).await;
                            match outcome {
                                PlayerOutcome::Nothing => {}
                                PlayerOutcome::Reply(msg) => send_msg(&mut ws_tx, &msg).await,
                                PlayerOutcome::BecameSpectator(tok) => {
                                    became_spectator = Some(tok);
                                }
                                PlayerOutcome::MovedToSlot(new_slot) => {
                                    player_index = new_slot;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
            msg = broadcast_rx.recv() => {
                match msg {
                    Ok((target, server_msg)) => {
                        let match_target = match became_spectator {
                            Some(_) => matches!(target, BroadcastTarget::AllSpectators),
                            None => target == BroadcastTarget::Player(player_index),
                        };
                        if match_target {
                            send_msg(&mut ws_tx, &server_msg).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("Player {player_index} lagged {n} messages");
                        let room_guard = room.lock().await;
                        if became_spectator.is_some() {
                            if let Some(game) = room_guard.game.as_ref() {
                                let state = game.get_observer_view();
                                let turn_deadline_secs = room_guard.turn_deadline_secs();
                                drop(room_guard);
                                send_msg(
                                    &mut ws_tx,
                                    &ServerMessage::GameState { state, turn_deadline_secs },
                                ).await;
                            }
                        } else if let Some(state) = room_guard.player_view(player_index) {
                            let turn_deadline_secs = room_guard.turn_deadline_secs();
                            drop(room_guard);
                            send_msg(
                                &mut ws_tx,
                                &ServerMessage::GameState { state, turn_deadline_secs },
                            ).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    // Disconnect path — branch on whether we're still seated or have
    // swapped to spectator.
    if let Some(spec_token) = &became_spectator {
        let mut room_guard = room.lock().await;
        if let Some(idx) = room_guard.find_spectator(spec_token) {
            room_guard.spectators[idx].connected = false;
            room_guard.spectators[idx].disconnected_at = Some(Instant::now());
        }
        room_guard.touch();
        room_guard.broadcast_lobby_state();
        schedule_spectator_cleanup(state.clone(), room.clone());
    } else {
        let mut room_guard = room.lock().await;
        room_guard.players[player_index].connected = false;
        room_guard.players[player_index].disconnected_at = Some(Instant::now());
        room_guard.touch();

        for (i, slot) in room_guard.players.iter().enumerate() {
            if i != player_index && slot.connected {
                let _ = room_guard.broadcast_tx.send((
                    BroadcastTarget::Player(i),
                    ServerMessage::PlayerLeft { player_index },
                ));
            }
        }

        if player_index == room_guard.creator {
            let room_clone = room.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(HOST_AUTO_PROMOTE_SECS)).await;
                let mut room_guard = room_clone.lock().await;
                if room_guard.auto_promote_host() {
                    room_guard.broadcast_lobby_state();
                }
            });
        }

        let room_clone = room.clone();
        let state_ref = state.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(DISCONNECT_AUTO_KICK_SECS)).await;
            let mut room_guard = room_clone.lock().await;
            let kicked = room_guard.auto_kick_disconnected(Duration::from_secs(
                DISCONNECT_AUTO_KICK_SECS,
            ));
            for (_, token) in &kicked {
                if let Some(t) = token {
                    state_ref.lobby.sessions.remove(t);
                }
            }
            if !kicked.is_empty() {
                room_guard.broadcast_lobby_state();
            }
        });
    }

    tracing::info!("Player {player_index} disconnected from room {room_code}");
}

pub async fn handle_spectator_ws(
    ws: WebSocket,
    state: AppState,
    room: SharedRoom,
    room_code: String,
    token: String,
) {
    let (mut ws_tx, mut ws_rx) = ws.split();

    let mut broadcast_rx = {
        let mut room_guard = room.lock().await;
        let idx = match room_guard.find_spectator(&token) {
            Some(i) => i,
            None => {
                send_msg(&mut ws_tx, &ServerMessage::Error {
                    code: "unknown_spectator".to_string(),
                    message: "Spectator session not found".to_string(),
                }).await;
                return;
            }
        };
        room_guard.spectators[idx].connected = true;
        room_guard.spectators[idx].disconnected_at = None;
        room_guard.touch();

        let initial = match room_guard.phase {
            RoomPhase::Lobby | RoomPhase::GameOver => ServerMessage::RoomState {
                state: room_guard.lobby_state(),
            },
            RoomPhase::InGame => match room_guard.game.as_ref() {
                Some(game) => ServerMessage::GameState {
                    state: game.get_observer_view(),
                    turn_deadline_secs: room_guard.turn_deadline_secs(),
                },
                None => ServerMessage::RoomState {
                    state: room_guard.lobby_state(),
                },
            },
        };
        send_msg(&mut ws_tx, &initial).await;

        // Announce to seated players that the spectator list changed.
        room_guard.broadcast_lobby_state();

        room_guard.broadcast_tx.subscribe()
    };

    loop {
        tokio::select! {
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let response = handle_spectator_message(
                            text.as_ref(),
                            &state,
                            &room,
                            &token,
                        ).await;
                        if let Some(msg) = response {
                            send_msg(&mut ws_tx, &msg).await;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
            msg = broadcast_rx.recv() => {
                match msg {
                    Ok((target, server_msg)) => {
                        if matches!(target, BroadcastTarget::AllSpectators) {
                            send_msg(&mut ws_tx, &server_msg).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        let room_guard = room.lock().await;
                        if let Some(game) = room_guard.game.as_ref() {
                            let state = game.get_observer_view();
                            let turn_deadline_secs = room_guard.turn_deadline_secs();
                            drop(room_guard);
                            send_msg(
                                &mut ws_tx,
                                &ServerMessage::GameState { state, turn_deadline_secs },
                            ).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    {
        let mut room_guard = room.lock().await;
        if let Some(idx) = room_guard.find_spectator(&token) {
            room_guard.spectators[idx].connected = false;
            room_guard.spectators[idx].disconnected_at = Some(Instant::now());
        }
        room_guard.touch();
        room_guard.broadcast_lobby_state();
    }

    schedule_spectator_cleanup(state.clone(), room.clone());

    tracing::info!("Spectator disconnected from room {room_code}");
}

/// Outcome of handling one inbound message from a seated player. Lets the
/// loop swap into spectator-filtering mode when the player drops into the
/// stands.
#[allow(clippy::large_enum_variant)]
enum PlayerOutcome {
    Nothing,
    Reply(ServerMessage),
    BecameSpectator(String),
    MovedToSlot(usize),
}

async fn handle_player_message(
    text: &str,
    state: &AppState,
    room: &SharedRoom,
    player_index: usize,
) -> PlayerOutcome {
    let msg: ClientMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            return PlayerOutcome::Reply(ServerMessage::Error {
                code: "invalid_message".to_string(),
                message: format!("Failed to parse message: {e}"),
            });
        }
    };

    match msg {
        ClientMessage::Ping => PlayerOutcome::Reply(ServerMessage::Pong),

        ClientMessage::ConfigureSlot { slot, player_type } => {
            let mut room_guard = room.lock().await;
            if player_index != room_guard.creator {
                return PlayerOutcome::Reply(not_creator("configure slots"));
            }
            match room_guard.configure_slot(slot, &player_type) {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    PlayerOutcome::Nothing
                }
                Err(e) => PlayerOutcome::Reply(err("configure_error", e)),
            }
        }

        ClientMessage::SetRules { rules } => {
            let mut room_guard = room.lock().await;
            if player_index != room_guard.creator {
                return PlayerOutcome::Reply(not_creator("change rules"));
            }
            match room_guard.set_rules(&rules) {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    PlayerOutcome::Nothing
                }
                Err(e) => PlayerOutcome::Reply(err("set_rules_error", e)),
            }
        }

        ClientMessage::SetNumPlayers { num_players } => {
            let mut room_guard = room.lock().await;
            if player_index != room_guard.creator {
                return PlayerOutcome::Reply(not_creator("change player count"));
            }
            match room_guard.set_num_players(num_players) {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    PlayerOutcome::Nothing
                }
                Err(e) => PlayerOutcome::Reply(err("set_players_error", e)),
            }
        }

        ClientMessage::KickPlayer { slot } => {
            let mut room_guard = room.lock().await;
            if player_index != room_guard.creator {
                return PlayerOutcome::Reply(not_creator("kick players"));
            }
            match room_guard.kick_player(slot) {
                Ok(token) => {
                    if let Some(t) = token {
                        state.lobby.sessions.remove(&t);
                    }
                    room_guard.broadcast_lobby_state();
                    PlayerOutcome::Nothing
                }
                Err(e) => PlayerOutcome::Reply(err("kick_error", e)),
            }
        }

        ClientMessage::PromoteHost { slot } => {
            let mut room_guard = room.lock().await;
            if player_index != room_guard.creator {
                return PlayerOutcome::Reply(not_creator("promote players"));
            }
            match room_guard.promote_host(slot) {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    PlayerOutcome::Nothing
                }
                Err(e) => PlayerOutcome::Reply(err("promote_error", e)),
            }
        }

        ClientMessage::SetTurnTimer { secs } => {
            let mut room_guard = room.lock().await;
            if player_index != room_guard.creator {
                return PlayerOutcome::Reply(not_creator("change the turn timer"));
            }
            match room_guard.set_turn_timer(secs) {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    PlayerOutcome::Nothing
                }
                Err(e) => PlayerOutcome::Reply(err("set_timer_error", e)),
            }
        }

        ClientMessage::StartGame => {
            let mut room_guard = room.lock().await;
            if player_index != room_guard.creator {
                return PlayerOutcome::Reply(not_creator("start the game"));
            }
            match room_guard.start_game() {
                Ok(()) => {
                    room_guard.broadcast_game_state();
                    if room_guard.is_current_player_bot() {
                        drop(room_guard);
                        let room_clone = room.clone();
                        tokio::spawn(async move { run_bot_turns(room_clone).await });
                    } else {
                        drop(room_guard);
                        schedule_turn_timeout(room.clone());
                    }
                    PlayerOutcome::Nothing
                }
                Err(e) => PlayerOutcome::Reply(err("start_error", e)),
            }
        }

        ClientMessage::Action { action } => {
            let mut room_guard = room.lock().await;
            match room_guard.apply_human_action(player_index, action.clone()) {
                Ok(()) => {
                    room_guard.broadcast_action(player_index, &action, false);
                    if room_guard.is_current_player_bot() {
                        drop(room_guard);
                        let room_clone = room.clone();
                        tokio::spawn(async move { run_bot_turns(room_clone).await });
                    } else {
                        drop(room_guard);
                        schedule_turn_timeout(room.clone());
                    }
                    PlayerOutcome::Nothing
                }
                Err(e) => PlayerOutcome::Reply(err("action_error", e)),
            }
        }

        ClientMessage::PlayAgain => {
            let mut room_guard = room.lock().await;
            if player_index != room_guard.creator {
                return PlayerOutcome::Reply(not_creator("restart the game"));
            }
            match room_guard.play_again() {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    PlayerOutcome::Nothing
                }
                Err(e) => PlayerOutcome::Reply(err("play_again_error", e)),
            }
        }

        ClientMessage::ReturnToLobby => {
            let mut room_guard = room.lock().await;
            match room_guard.return_to_lobby() {
                Ok(()) => {
                    room_guard.broadcast_lobby_state();
                    PlayerOutcome::Nothing
                }
                Err(e) => PlayerOutcome::Reply(err("return_error", e)),
            }
        }

        ClientMessage::TakeSlot { slot } => {
            let mut room_guard = room.lock().await;
            match room_guard.take_slot(player_index, slot) {
                Ok(()) => {
                    // Re-point the session token to the new slot.
                    if let Some(tok) = room_guard.players[slot].session_token.clone() {
                        state.lobby.sessions.insert(
                            tok.to_string(),
                            SessionRef::Player {
                                code: room_guard.code.clone(),
                                slot,
                            },
                        );
                    }
                    room_guard.broadcast_lobby_state();
                    PlayerOutcome::MovedToSlot(slot)
                }
                Err(e) => PlayerOutcome::Reply(err("take_slot_error", e)),
            }
        }

        ClientMessage::BecomeSpectator => {
            let mut room_guard = room.lock().await;
            match room_guard.become_spectator(player_index) {
                Ok(tok) => {
                    state.lobby.sessions.insert(
                        tok.clone(),
                        SessionRef::Spectator {
                            code: room_guard.code.clone(),
                        },
                    );
                    room_guard.broadcast_lobby_state();
                    PlayerOutcome::BecameSpectator(tok)
                }
                Err(e) => PlayerOutcome::Reply(err("become_spectator_error", e)),
            }
        }
    }
}

async fn handle_spectator_message(
    text: &str,
    state: &AppState,
    room: &SharedRoom,
    token: &str,
) -> Option<ServerMessage> {
    let msg: ClientMessage = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(e) => {
            return Some(ServerMessage::Error {
                code: "invalid_message".to_string(),
                message: format!("Failed to parse message: {e}"),
            });
        }
    };

    match msg {
        ClientMessage::Ping => Some(ServerMessage::Pong),
        ClientMessage::TakeSlot { slot } => {
            let mut room_guard = room.lock().await;
            let spec_idx = match room_guard.find_spectator(token) {
                Some(i) => i,
                None => return Some(err("unknown_spectator", "Spectator session not found".to_string())),
            };
            match room_guard.spectator_take_slot(spec_idx, slot) {
                Ok(new_slot) => {
                    state.lobby.sessions.insert(
                        token.to_string(),
                        SessionRef::Player {
                            code: room_guard.code.clone(),
                            slot: new_slot,
                        },
                    );
                    room_guard.broadcast_lobby_state();
                    Some(ServerMessage::Error {
                        code: "role_changed".to_string(),
                        message: "You've taken a seat. Reconnect to play.".to_string(),
                    })
                }
                Err(e) => Some(err("take_slot_error", e)),
            }
        }
        _ => Some(ServerMessage::Error {
            code: "spectator_only".to_string(),
            message: "Spectators cannot send that message".to_string(),
        }),
    }
}

fn not_creator(action: &str) -> ServerMessage {
    ServerMessage::Error {
        code: "not_creator".to_string(),
        message: format!("Only the room creator can {action}"),
    }
}

fn err(code: &str, message: String) -> ServerMessage {
    ServerMessage::Error {
        code: code.to_string(),
        message,
    }
}

fn schedule_spectator_cleanup(state: AppState, room: SharedRoom) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(SPECTATOR_AUTO_KICK_SECS)).await;
        let mut room_guard = room.lock().await;
        let expired = room_guard
            .auto_kick_disconnected_spectators(Duration::from_secs(SPECTATOR_AUTO_KICK_SECS));
        for t in &expired {
            state.lobby.sessions.remove(t);
        }
        if !expired.is_empty() {
            room_guard.broadcast_lobby_state();
        }
    });
}

/// Watch the current human player's turn. If they exceed the timer, apply a
/// random action on their behalf and continue the flow (bot turn loop or the
/// next human's timer).
fn schedule_turn_timeout(room: SharedRoom) {
    tokio::spawn(async move {
        let timer_secs = {
            let room_guard = room.lock().await;
            match room_guard.turn_timer_secs {
                Some(s) => s,
                None => return,
            }
        };
        // +1s buffer absorbs scheduler jitter.
        tokio::time::sleep(Duration::from_secs(timer_secs + 1)).await;

        let mut room_guard = room.lock().await;
        match room_guard.check_turn_timeout() {
            Ok(Some((player, action))) => {
                room_guard.broadcast_timeout_action(player, &action);
                if room_guard.is_current_player_bot() {
                    drop(room_guard);
                    let room_clone = room.clone();
                    tokio::spawn(async move { run_bot_turns(room_clone).await });
                } else {
                    drop(room_guard);
                    schedule_turn_timeout(room.clone());
                }
            }
            Ok(None) => {}
            Err(e) => tracing::error!("Turn timeout check failed: {e}"),
        }
    });
}

/// Drive consecutive bot turns with a short visual delay. Stops on a human
/// turn (scheduling the timer) or on game over.
async fn run_bot_turns(room: SharedRoom) {
    loop {
        tokio::time::sleep(Duration::from_millis(BOT_TURN_DELAY_MS)).await;
        let mut room_guard = room.lock().await;
        if !room_guard.is_current_player_bot() {
            drop(room_guard);
            schedule_turn_timeout(room.clone());
            break;
        }
        match room_guard.apply_bot_action() {
            Ok((bot_player, action)) => {
                room_guard.broadcast_action(bot_player, &action, true);
            }
            Err(e) => {
                tracing::error!("Bot action failed: {e}");
                break;
            }
        }
    }
}

async fn send_msg(tx: &mut SplitSink<WebSocket, Message>, msg: &ServerMessage) {
    if let Ok(json) = serde_json::to_string(msg) {
        let _ = tx.send(Message::Text(json.into())).await;
    }
}
