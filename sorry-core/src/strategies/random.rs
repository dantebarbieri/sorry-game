use rand::RngCore;
use rand::seq::SliceRandom;

use crate::card::Card;
use crate::moves::Move;
use crate::rules::Rules;
use crate::strategy::{Complexity, Strategy, StrategyDescription, StrategyView};

#[derive(Debug, Default, Clone, Copy)]
pub struct RandomStrategy;

impl Strategy for RandomStrategy {
    fn name(&self) -> &str {
        "Random"
    }

    fn describe(&self) -> StrategyDescription {
        StrategyDescription {
            name: self.name().to_string(),
            summary: "Picks a uniformly random legal move.".to_string(),
            complexity: Complexity::Trivial,
        }
    }

    fn choose_card(&self, view: &StrategyView, _rules: &dyn Rules, rng: &mut dyn RngCore) -> usize {
        if view.hand.is_empty() {
            0
        } else {
            (rng.next_u32() as usize) % view.hand.len()
        }
    }

    fn choose_move(
        &self,
        _view: &StrategyView,
        _rules: &dyn Rules,
        _card: Card,
        legal: &[Move],
        rng: &mut dyn RngCore,
    ) -> Move {
        legal.choose(rng).cloned().unwrap_or(Move::Pass)
    }
}
