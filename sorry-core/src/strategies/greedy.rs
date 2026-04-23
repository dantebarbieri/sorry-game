use rand::RngCore;

use crate::card::Card;
use crate::moves::Move;
use crate::rules::Rules;
use crate::strategy::{Complexity, Strategy, StrategyDescription, StrategyView};

use super::util::greedy_pick;

#[derive(Debug, Default, Clone, Copy)]
pub struct GreedyStrategy;

impl Strategy for GreedyStrategy {
    fn name(&self) -> &str {
        "Greedy"
    }

    fn describe(&self) -> StrategyDescription {
        StrategyDescription {
            name: self.name().to_string(),
            summary: "Always minimizes total distance-to-home, preferring moves into the safety zone.".to_string(),
            complexity: Complexity::Low,
        }
    }

    fn choose_move(
        &self,
        view: &StrategyView,
        rules: &dyn Rules,
        _card: Card,
        legal: &[Move],
        rng: &mut dyn RngCore,
    ) -> Move {
        greedy_pick(view, rules, legal, rng)
    }
}
