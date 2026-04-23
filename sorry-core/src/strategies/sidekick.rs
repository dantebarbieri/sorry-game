use rand::RngCore;
use rand::seq::SliceRandom;

use crate::card::Card;
use crate::moves::Move;
use crate::rules::Rules;
use crate::strategy::{Complexity, Strategy, StrategyDescription, StrategyView};

use super::util::{bumps_opponent, greedy_pick, landing_squares};

/// "Sidekick" — pile on whichever opponent most recently had a pawn
/// bumped. If no recent victim is on record, or the victim has no pawns
/// left outside StartArea, fall through to Greedy.
#[derive(Debug, Default, Clone, Copy)]
pub struct SidekickStrategy;

impl Strategy for SidekickStrategy {
    fn name(&self) -> &str {
        "Sidekick"
    }

    fn describe(&self) -> StrategyDescription {
        StrategyDescription {
            name: self.name().to_string(),
            summary: "Attacks the most recently-bumped opponent; Greedy otherwise.".to_string(),
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
        let victim = match view.last_bump_victim {
            Some(v) if v != me => v,
            _ => return greedy_pick(view, rules, legal, rng),
        };
        // If the victim has no pawns anywhere but their own StartArea,
        // there's nothing to attack.
        let victim_start = rules.start_area(victim);
        let any_outside = view.pawn_positions[victim.0 as usize]
            .iter()
            .any(|s| *s != victim_start);
        if !any_outside {
            return greedy_pick(view, rules, legal, rng);
        }

        // Same priority as Survivor, but pointed at `victim`.
        let bumpers: Vec<&Move> = legal
            .iter()
            .filter(|m| matches!(m, Move::Advance { .. } | Move::Retreat { .. }))
            .filter(|m| {
                landing_squares(m, view, me)
                    .iter()
                    .any(|sq| matches!(bumps_opponent(view, me, *sq), Some((p, _)) if p == victim))
            })
            .collect();
        if let Some(m) = bumpers.choose(rng) {
            return (*m).clone();
        }
        let sorries: Vec<&Move> = legal
            .iter()
            .filter(|m| matches!(m, Move::Sorry { their_player, .. } if *their_player == victim))
            .collect();
        if let Some(m) = sorries.choose(rng) {
            return (*m).clone();
        }
        let swaps: Vec<&Move> = legal
            .iter()
            .filter(
                |m| matches!(m, Move::SwapEleven { their_player, .. } if *their_player == victim),
            )
            .collect();
        if let Some(m) = swaps.choose(rng) {
            return (*m).clone();
        }
        greedy_pick(view, rules, legal, rng)
    }
}
