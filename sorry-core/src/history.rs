use serde::{Deserialize, Serialize};

use crate::board::PlayerId;
use crate::card::Card;
use crate::moves::{BumpEvent, Move, SlideEvent};

/// Full deterministic record of a game. `initial_deck_order` + `seed` +
/// `turns` are sufficient to reconstruct every intermediate state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameHistory {
    pub seed: u64,
    pub num_players: usize,
    pub strategy_names: Vec<String>,
    pub rules_name: String,
    pub initial_deck_order: Vec<Card>,
    pub starting_player: PlayerId,
    pub turns: Vec<TurnRecord>,
    pub winners: Vec<PlayerId>,
    #[serde(default)]
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnRecord {
    pub player: PlayerId,
    pub actions: Vec<Action>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Action {
    Draw {
        card: Card,
    },
    /// Skipped when `Rules::hand_size() == 0`.
    ChooseCard {
        hand_index: usize,
        card: Card,
    },
    Play {
        card: Card,
        mv: Move,
        bumps: Vec<BumpEvent>,
        slides: Vec<SlideEvent>,
    },
    /// Deck ran out and was reshuffled from the discard pile.
    Reshuffle,
    /// The card granted an extra turn (only 2 in standard rules).
    ExtraTurnGranted,
}
