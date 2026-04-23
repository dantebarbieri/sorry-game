use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::board::{PlayerId, SpaceId};
use crate::card::Card;
use crate::moves::Move;
use crate::rules::Rules;

/// Public information available to a strategy when it is asked to make a
/// decision. Sorry! has no hidden state (all pawn positions are public), so
/// the view is a straightforward snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyView {
    pub my_player: PlayerId,
    pub num_players: usize,
    /// Persistent hand. Empty when `Rules::hand_size() == 0`.
    pub hand: Vec<Card>,
    /// The card just drawn for this turn. Only set when `hand_size() == 0`.
    pub drawn_card: Option<Card>,
    /// `pawn_positions[player][pawn]` → SpaceId. All public in Sorry!.
    pub pawn_positions: Vec<Vec<SpaceId>>,
    /// Full discard pile contents, oldest first.
    pub discard: Vec<Card>,
    pub deck_remaining: usize,
    pub current_player_turn: PlayerId,
    /// The most recent player to have a pawn bumped back to their Start
    /// area — by any means (normal landing bump, slide traversal, or a
    /// Sorry card). Set once per game start to `None` and overwritten as
    /// bumps happen. Used by strategies like `Sidekick` that target a
    /// recently-weakened opponent.
    #[serde(default)]
    pub last_bump_victim: Option<PlayerId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyDescription {
    pub name: String,
    pub summary: String,
    pub complexity: Complexity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Complexity {
    Trivial,
    Low,
    Medium,
    High,
}

pub trait Strategy: Send + Sync {
    fn name(&self) -> &str;

    fn describe(&self) -> StrategyDescription {
        StrategyDescription {
            name: self.name().to_string(),
            summary: String::new(),
            complexity: Complexity::Trivial,
        }
    }

    /// Only invoked when `Rules::hand_size() > 1`. Returns an index into
    /// `view.hand`. Default implementation plays the first card.
    fn choose_card(&self, view: &StrategyView, rules: &dyn Rules, rng: &mut dyn RngCore) -> usize {
        let _ = (view, rules, rng);
        0
    }

    /// Pick a move from the provided legal set. Implementations MUST return
    /// one of the moves in `legal` (the engine rejects foreign moves).
    fn choose_move(
        &self,
        view: &StrategyView,
        rules: &dyn Rules,
        card: Card,
        legal: &[Move],
        rng: &mut dyn RngCore,
    ) -> Move;
}
