use std::sync::Arc;
use std::time::{Duration, Instant};

use rand::Rng;
use tokio::sync::{Mutex, broadcast};

use sorry_core::{ActionNeeded, InteractiveGame, PlayerAction, PlayerId, PlayerView};

use crate::messages::{LobbyPlayer, PlayerSlotType, RoomLobbyState, ServerMessage};
use crate::session::SessionToken;
use crate::strategies::{available_rules, available_strategies, make_rules, make_strategy};

const LOBBY_IDLE_TIMEOUT_SECS: u64 = 30 * 60;
const BROADCAST_CAPACITY: usize = 256;
const DEFAULT_TURN_TIMER_SECS: u64 = 60;

#[derive(Debug, Clone, PartialEq)]
pub enum RoomPhase {
    Lobby,
    InGame,
    GameOver,
}

#[derive(Debug, Clone)]
pub struct PlayerSlot {
    pub name: String,
    pub slot_type: PlayerSlotType,
    pub session_token: Option<SessionToken>,
    pub connected: bool,
    pub disconnected_at: Option<Instant>,
}

impl PlayerSlot {
    fn empty() -> Self {
        Self {
            name: String::new(),
            slot_type: PlayerSlotType::Empty,
            session_token: None,
            connected: false,
            disconnected_at: None,
        }
    }
}

pub struct Room {
    pub code: String,
    pub phase: RoomPhase,
    pub num_players: usize,
    pub rules_name: String,
    pub creator: usize,
    pub players: Vec<PlayerSlot>,
    pub game: Option<InteractiveGame>,
    pub last_activity: Instant,
    pub broadcast_tx: broadcast::Sender<(usize, ServerMessage)>,
    pub last_winners: Vec<usize>,
    pub turn_timer_secs: Option<u64>,
    pub turn_start: Option<Instant>,
}

pub type SharedRoom = Arc<Mutex<Room>>;

impl Room {
    pub fn new(
        code: String,
        creator_name: String,
        num_players: usize,
        rules: Option<String>,
    ) -> Self {
        let (broadcast_tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        let rules_name = rules.unwrap_or_else(|| "Standard".to_string());

        let mut players = Vec::with_capacity(num_players);
        players.push(PlayerSlot {
            name: creator_name,
            slot_type: PlayerSlotType::Human,
            session_token: None,
            connected: false,
            disconnected_at: None,
        });
        for _ in 1..num_players {
            players.push(PlayerSlot::empty());
        }

        Room {
            code,
            phase: RoomPhase::Lobby,
            num_players,
            rules_name,
            creator: 0,
            players,
            game: None,
            last_activity: Instant::now(),
            broadcast_tx,
            last_winners: Vec::new(),
            turn_timer_secs: Some(DEFAULT_TURN_TIMER_SECS),
            turn_start: None,
        }
    }

    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
    }

    pub fn phase_label(&self) -> &'static str {
        match self.phase {
            RoomPhase::Lobby => "lobby",
            RoomPhase::InGame => "in_game",
            RoomPhase::GameOver => "game_over",
        }
    }

    pub fn next_available_slot(&self) -> Option<usize> {
        self.players
            .iter()
            .position(|p| p.slot_type == PlayerSlotType::Empty)
            .or_else(|| {
                self.players
                    .iter()
                    .position(|p| matches!(p.slot_type, PlayerSlotType::Bot { .. }))
            })
    }

    pub fn configure_slot(&mut self, slot: usize, player_type: &str) -> Result<(), String> {
        if self.phase != RoomPhase::Lobby {
            return Err("Cannot configure slots during game".to_string());
        }
        if slot >= self.num_players {
            return Err("Invalid slot".to_string());
        }

        match player_type {
            "Empty" => {
                if slot == self.creator {
                    return Err("Cannot remove the creator".to_string());
                }
                if matches!(self.players[slot].slot_type, PlayerSlotType::Human) {
                    return Err("Use KickPlayer to remove a human player".to_string());
                }
                self.players[slot] = PlayerSlot::empty();
            }
            s if s.starts_with("Bot:") => {
                let strategy = &s[4..];
                make_strategy(strategy)?;
                if matches!(self.players[slot].slot_type, PlayerSlotType::Human) {
                    return Err("Use KickPlayer to remove a human before assigning a bot".to_string());
                }
                self.players[slot] = PlayerSlot {
                    name: format!("Bot ({strategy})"),
                    slot_type: PlayerSlotType::Bot {
                        strategy: strategy.to_string(),
                    },
                    session_token: None,
                    connected: false,
                    disconnected_at: None,
                };
            }
            _ => return Err(format!("Unknown player type: {player_type}")),
        }

        self.touch();
        Ok(())
    }

    pub fn set_rules(&mut self, rules: &str) -> Result<(), String> {
        if self.phase != RoomPhase::Lobby {
            return Err("Cannot change rules during game".to_string());
        }
        make_rules(rules)?;
        self.rules_name = rules.to_string();
        self.touch();
        Ok(())
    }

    pub fn set_num_players(&mut self, num_players: usize) -> Result<(), String> {
        if self.phase != RoomPhase::Lobby {
            return Err("Cannot change player count during game".to_string());
        }
        if !(2..=4).contains(&num_players) {
            return Err("Player count must be 2-4".to_string());
        }

        if num_players > self.num_players {
            for _ in self.num_players..num_players {
                self.players.push(PlayerSlot::empty());
            }
        } else if num_players < self.num_players {
            for i in (num_players..self.num_players).rev() {
                if self.players[i].slot_type != PlayerSlotType::Empty {
                    return Err(format!(
                        "Cannot reduce to {num_players} players: slot {} is occupied",
                        i + 1
                    ));
                }
            }
            self.players.truncate(num_players);
        }

        self.num_players = num_players;
        self.touch();
        Ok(())
    }

    pub fn set_turn_timer(&mut self, secs: Option<u64>) -> Result<(), String> {
        if self.phase != RoomPhase::Lobby {
            return Err("Cannot change turn timer during game".to_string());
        }
        if matches!(secs, Some(0)) {
            return Err("Turn timer must be positive".to_string());
        }
        self.turn_timer_secs = secs;
        self.touch();
        Ok(())
    }

    pub fn kick_player(&mut self, slot: usize) -> Result<Option<String>, String> {
        if self.phase != RoomPhase::Lobby {
            return Err("Cannot kick players during game".to_string());
        }
        if slot >= self.num_players {
            return Err("Invalid slot".to_string());
        }
        if slot == self.creator {
            return Err("Cannot kick the room creator".to_string());
        }
        if self.players[slot].slot_type == PlayerSlotType::Empty {
            return Err("Slot is already empty".to_string());
        }

        let token = self.players[slot]
            .session_token
            .as_ref()
            .map(|t| t.to_string());

        let _ = self.broadcast_tx.send((
            slot,
            ServerMessage::Kicked {
                reason: "You were kicked by the room host".to_string(),
            },
        ));

        self.players[slot] = PlayerSlot::empty();
        self.touch();
        Ok(token)
    }

    pub fn promote_host(&mut self, slot: usize) -> Result<(), String> {
        if slot >= self.num_players {
            return Err("Invalid slot".to_string());
        }
        if !matches!(self.players[slot].slot_type, PlayerSlotType::Human) {
            return Err("Can only promote human players".to_string());
        }
        self.creator = slot;
        self.touch();
        Ok(())
    }

    /// After the creator disconnects, pick a still-connected human host.
    /// Returns `true` if the creator slot changed.
    pub fn auto_promote_host(&mut self) -> bool {
        if self.players[self.creator].connected {
            return false;
        }
        for i in 0..self.num_players {
            if i != self.creator
                && self.players[i].connected
                && matches!(self.players[i].slot_type, PlayerSlotType::Human)
            {
                self.creator = i;
                self.touch();
                return true;
            }
        }
        false
    }

    /// Reclaim human slots whose occupant has been disconnected longer than
    /// `timeout`. Returns `(slot, session_token)` pairs so the lobby can
    /// expire their sessions.
    pub fn auto_kick_disconnected(
        &mut self,
        timeout: Duration,
    ) -> Vec<(usize, Option<String>)> {
        // Don't reap mid-game; the game state would become inconsistent.
        if self.phase != RoomPhase::Lobby {
            return Vec::new();
        }
        let mut kicked = Vec::new();
        for i in 0..self.num_players {
            if i == self.creator {
                continue;
            }
            if let Some(dc_at) = self.players[i].disconnected_at
                && dc_at.elapsed() >= timeout
                && matches!(self.players[i].slot_type, PlayerSlotType::Human)
            {
                let token = self.players[i].session_token.as_ref().map(|t| t.to_string());
                let _ = self.broadcast_tx.send((
                    i,
                    ServerMessage::Kicked {
                        reason: "Disconnected for too long".to_string(),
                    },
                ));
                self.players[i] = PlayerSlot::empty();
                kicked.push((i, token));
            }
        }
        if !kicked.is_empty() {
            self.touch();
        }
        kicked
    }

    pub fn all_slots_filled(&self) -> bool {
        self.players
            .iter()
            .all(|p| p.slot_type != PlayerSlotType::Empty)
    }

    pub fn start_game(&mut self) -> Result<(), String> {
        if self.phase != RoomPhase::Lobby {
            return Err("Game already started".to_string());
        }
        if !self.all_slots_filled() {
            return Err("Not all player slots are filled".to_string());
        }

        let rules = make_rules(&self.rules_name)?;
        // strategy_names carry "Human" or the bot's strategy name — useful for
        // replay/debug via GameHistory.strategy_names.
        let strategy_names: Vec<String> = self
            .players
            .iter()
            .map(|p| match &p.slot_type {
                PlayerSlotType::Human => "Human".to_string(),
                PlayerSlotType::Bot { strategy } => strategy.clone(),
                PlayerSlotType::Empty => "Empty".to_string(),
            })
            .collect();
        let seed: u64 = rand::rng().random();

        let game = InteractiveGame::new_with_strategy_names(rules, strategy_names, seed)
            .map_err(|e| e.to_string())?;

        self.game = Some(game);
        self.phase = RoomPhase::InGame;
        self.last_winners.clear();
        self.touch();
        self.reset_turn_start();
        Ok(())
    }

    pub fn play_again(&mut self) -> Result<(), String> {
        if self.phase != RoomPhase::GameOver {
            return Err("Game is not over".to_string());
        }
        self.game = None;
        self.phase = RoomPhase::Lobby;
        self.turn_start = None;
        self.touch();
        Ok(())
    }

    pub fn return_to_lobby(&mut self) -> Result<(), String> {
        if self.phase != RoomPhase::GameOver {
            return Err("Game is not over".to_string());
        }
        self.game = None;
        self.phase = RoomPhase::Lobby;
        self.turn_start = None;
        self.touch();
        Ok(())
    }

    /// `PlayerId` of the player whose turn it is, if any.
    pub fn current_player_index(&self) -> Option<usize> {
        let game = self.game.as_ref()?;
        match game.action_needed() {
            ActionNeeded::GameOver { .. } => None,
            ActionNeeded::ChooseCard { player, .. } | ActionNeeded::ChooseMove { player, .. } => {
                Some(player.0 as usize)
            }
        }
    }

    pub fn is_current_player_bot(&self) -> bool {
        match self.current_player_index() {
            Some(idx) => matches!(self.players[idx].slot_type, PlayerSlotType::Bot { .. }),
            None => false,
        }
    }

    pub fn apply_human_action(
        &mut self,
        player_index: usize,
        action: PlayerAction,
    ) -> Result<(), String> {
        {
            let game = self.game.as_mut().ok_or("No active game")?;
            match game.action_needed() {
                ActionNeeded::GameOver { .. } => {
                    return Err("Game is over".to_string());
                }
                ActionNeeded::ChooseCard { player, .. }
                | ActionNeeded::ChooseMove { player, .. } => {
                    if player.0 as usize != player_index {
                        return Err(format!(
                            "Not your turn: expected player {}, got {player_index}",
                            player.0
                        ));
                    }
                }
            }
            game.apply_action(action).map_err(|e| e.to_string())?;
        }
        self.touch();
        self.check_game_over();
        self.reset_turn_start();
        Ok(())
    }

    pub fn apply_bot_action(&mut self) -> Result<(usize, PlayerAction), String> {
        let (current, action) = {
            let game = self.game.as_mut().ok_or("No active game")?;
            let current = match game.action_needed() {
                ActionNeeded::ChooseCard { player, .. }
                | ActionNeeded::ChooseMove { player, .. } => player.0 as usize,
                ActionNeeded::GameOver { .. } => return Err("Game is over".to_string()),
            };
            let strategy_name = match &self.players[current].slot_type {
                PlayerSlotType::Bot { strategy } => strategy.clone(),
                _ => return Err("Current player is not a bot".to_string()),
            };
            let strategy = make_strategy(&strategy_name)?;
            let action = game.get_bot_action(strategy.as_ref()).map_err(|e| e.to_string())?;
            game.apply_action(action.clone()).map_err(|e| e.to_string())?;
            (current, action)
        };
        self.touch();
        self.check_game_over();
        self.reset_turn_start();
        Ok((current, action))
    }

    fn check_game_over(&mut self) {
        if let Some(game) = &self.game
            && let ActionNeeded::GameOver { winners, .. } = game.action_needed()
        {
            self.phase = RoomPhase::GameOver;
            self.last_winners = winners.iter().map(|p| p.0 as usize).collect();
            self.turn_start = None;
        }
    }

    pub fn reset_turn_start(&mut self) {
        if self.turn_timer_secs.is_none() || self.phase != RoomPhase::InGame {
            self.turn_start = None;
            return;
        }
        match self.current_player_index() {
            Some(idx) if matches!(self.players[idx].slot_type, PlayerSlotType::Human) => {
                self.turn_start = Some(Instant::now());
            }
            _ => self.turn_start = None,
        }
    }

    pub fn turn_deadline_secs(&self) -> Option<u64> {
        let timer = self.turn_timer_secs?;
        let start = self.turn_start?;
        Some(timer.saturating_sub(start.elapsed().as_secs()))
    }

    /// If the current human's turn has exceeded the timer, pick a random legal
    /// action and apply it. Returns the applied `(player, action)` when a
    /// timeout fired.
    pub fn check_turn_timeout(&mut self) -> Result<Option<(usize, PlayerAction)>, String> {
        let timer = match self.turn_timer_secs {
            Some(t) => t,
            None => return Ok(None),
        };
        let start = match self.turn_start {
            Some(s) => s,
            None => return Ok(None),
        };
        if start.elapsed().as_secs() < timer {
            return Ok(None);
        }

        let (current, action) = {
            let game = self.game.as_mut().ok_or("No active game")?;
            let current = match game.action_needed() {
                ActionNeeded::ChooseCard { player, .. }
                | ActionNeeded::ChooseMove { player, .. } => player.0 as usize,
                ActionNeeded::GameOver { .. } => return Ok(None),
            };
            // Pull a random move from the RandomStrategy, same as skyjo.
            let strategy = sorry_core::RandomStrategy;
            let action = game.get_bot_action(&strategy).map_err(|e| e.to_string())?;
            game.apply_action(action.clone()).map_err(|e| e.to_string())?;
            (current, action)
        };

        self.touch();
        self.check_game_over();
        self.reset_turn_start();
        Ok(Some((current, action)))
    }

    pub fn player_view(&self, viewer: usize) -> Option<PlayerView> {
        let game = self.game.as_ref()?;
        Some(game.get_player_view(PlayerId(viewer as u8)))
    }

    pub fn lobby_state(&self) -> RoomLobbyState {
        let players: Vec<LobbyPlayer> = self
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| LobbyPlayer {
                slot: i,
                name: p.name.clone(),
                player_type: p.slot_type.clone(),
                connected: p.connected,
                disconnect_secs: p.disconnected_at.map(|t| t.elapsed().as_secs()),
            })
            .collect();

        let idle_timeout_secs = if self.phase == RoomPhase::Lobby {
            Some(LOBBY_IDLE_TIMEOUT_SECS.saturating_sub(self.last_activity.elapsed().as_secs()))
        } else {
            None
        };

        RoomLobbyState {
            room_code: self.code.clone(),
            phase: self.phase_label().to_string(),
            players,
            num_players: self.num_players,
            rules: self.rules_name.clone(),
            creator: self.creator,
            available_strategies: available_strategies(),
            available_rules: available_rules(),
            idle_timeout_secs,
            turn_timer_secs: self.turn_timer_secs,
            last_winners: self.last_winners.clone(),
        }
    }

    pub fn broadcast_lobby_state(&self) {
        let state = self.lobby_state();
        for (i, slot) in self.players.iter().enumerate() {
            if slot.connected {
                let _ = self.broadcast_tx.send((
                    i,
                    ServerMessage::RoomState {
                        state: state.clone(),
                    },
                ));
            }
        }
    }

    pub fn broadcast_game_state(&self) {
        let game = match &self.game {
            Some(g) => g,
            None => return,
        };
        let deadline = self.turn_deadline_secs();
        for (i, slot) in self.players.iter().enumerate() {
            if slot.connected && matches!(slot.slot_type, PlayerSlotType::Human) {
                let state = game.get_player_view(PlayerId(i as u8));
                let _ = self.broadcast_tx.send((
                    i,
                    ServerMessage::GameState {
                        state,
                        turn_deadline_secs: deadline,
                    },
                ));
            }
        }
    }

    pub fn broadcast_action(&self, player: usize, action: &PlayerAction, is_bot: bool) {
        let game = match &self.game {
            Some(g) => g,
            None => return,
        };
        let deadline = self.turn_deadline_secs();
        for (i, slot) in self.players.iter().enumerate() {
            if slot.connected && matches!(slot.slot_type, PlayerSlotType::Human) {
                let state = game.get_player_view(PlayerId(i as u8));
                let msg = if is_bot {
                    ServerMessage::BotAction {
                        player,
                        action: action.clone(),
                        state,
                        turn_deadline_secs: deadline,
                    }
                } else {
                    ServerMessage::ActionApplied {
                        player,
                        action: action.clone(),
                        state,
                        turn_deadline_secs: deadline,
                    }
                };
                let _ = self.broadcast_tx.send((i, msg));
            }
        }
    }

    pub fn broadcast_timeout_action(&self, player: usize, action: &PlayerAction) {
        let game = match &self.game {
            Some(g) => g,
            None => return,
        };
        for (i, slot) in self.players.iter().enumerate() {
            if slot.connected && matches!(slot.slot_type, PlayerSlotType::Human) {
                let state = game.get_player_view(PlayerId(i as u8));
                let _ = self.broadcast_tx.send((
                    i,
                    ServerMessage::TimeoutAction {
                        player,
                        action: action.clone(),
                        state,
                    },
                ));
            }
        }
    }
}
