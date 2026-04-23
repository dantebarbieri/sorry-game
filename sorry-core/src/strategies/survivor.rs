use rand::RngCore;
use rand::seq::SliceRandom;

use crate::card::Card;
use crate::moves::Move;
use crate::rules::Rules;
use crate::strategy::{Complexity, Strategy, StrategyDescription, StrategyView};

use super::util::{bumps_opponent, greedy_pick, landing_squares, leader};

/// "Survivor" — attacks the current leader (smallest mean distance-to-home
/// among opponents). Priority: direct bump > Sorry-leader > 11-swap leader
/// > Greedy fallback.
#[derive(Debug, Default, Clone, Copy)]
pub struct SurvivorStrategy;

impl Strategy for SurvivorStrategy {
    fn name(&self) -> &str {
        "Survivor"
    }

    fn describe(&self) -> StrategyDescription {
        StrategyDescription {
            name: self.name().to_string(),
            summary: "Attacks the current leader (bump > Sorry > 11-swap), Greedy otherwise."
                .to_string(),
            complexity: Complexity::Medium,
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
        let me = view.my_player;
        let target = match leader(view, rules, me) {
            Some(p) => p,
            None => return greedy_pick(view, rules, legal, rng),
        };

        // 1. Direct bump — any Advance/Retreat that lands on a leader pawn.
        let bumpers: Vec<&Move> = legal
            .iter()
            .filter(|m| matches!(m, Move::Advance { .. } | Move::Retreat { .. }))
            .filter(|m| {
                landing_squares(m, view, me)
                    .iter()
                    .any(|sq| matches!(bumps_opponent(view, me, *sq), Some((p, _)) if p == target))
            })
            .collect();
        if let Some(m) = bumpers.choose(rng) {
            return (*m).clone();
        }

        // 2. Sorry leader.
        let sorries: Vec<&Move> = legal
            .iter()
            .filter(|m| matches!(m, Move::Sorry { their_player, .. } if *their_player == target))
            .collect();
        if let Some(m) = sorries.choose(rng) {
            return (*m).clone();
        }

        // 3. 11-swap leader.
        let swaps: Vec<&Move> = legal
            .iter()
            .filter(
                |m| matches!(m, Move::SwapEleven { their_player, .. } if *their_player == target),
            )
            .collect();
        if let Some(m) = swaps.choose(rng) {
            return (*m).clone();
        }

        greedy_pick(view, rules, legal, rng)
    }
}
