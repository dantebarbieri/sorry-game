use rand::RngCore;
use rand::seq::SliceRandom;

use crate::board::SpaceId;
use crate::card::Card;
use crate::moves::Move;
use crate::rules::Rules;
use crate::strategy::{Complexity, Strategy, StrategyDescription, StrategyView};

use super::util::greedy_pick;

/// "Teleporter" — always swaps on 11 when a swap is legal, even when the
/// swap moves its own pawn backward. Picks the swap that maximizes the
/// forward-distance of its own pawn from `start_exit` after the swap.
/// Greedy fallback otherwise.
#[derive(Debug, Default, Clone, Copy)]
pub struct TeleporterStrategy;

impl Strategy for TeleporterStrategy {
    fn name(&self) -> &str {
        "Teleporter"
    }

    fn describe(&self) -> StrategyDescription {
        StrategyDescription {
            name: self.name().to_string(),
            summary: "Always swaps on 11; picks the swap that best advances own pawn; Greedy otherwise.".to_string(),
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
        let me = view.my_player;
        let swaps: Vec<&Move> = legal
            .iter()
            .filter(|m| matches!(m, Move::SwapEleven { .. }))
            .collect();
        if swaps.is_empty() {
            return greedy_pick(view, rules, legal, rng);
        }
        // Score each swap by forward distance of our pawn from start_exit
        // AFTER the swap (i.e., how many steps past start_exit the target
        // opponent's current position is for our player).
        let start_exit = rules.start_exit(me);
        let scored: Vec<(i64, &Move)> = swaps
            .iter()
            .map(|m| {
                let (their_player, their_pawn) = match m {
                    Move::SwapEleven {
                        their_player,
                        their_pawn,
                        ..
                    } => (*their_player, *their_pawn),
                    _ => unreachable!(),
                };
                let their_pos =
                    view.pawn_positions[their_player.0 as usize][their_pawn.0 as usize];
                (forward_steps(rules, start_exit, their_pos, me), *m)
            })
            .collect();
        let best = scored.iter().map(|(d, _)| *d).max().unwrap();
        let candidates: Vec<&Move> =
            scored.into_iter().filter_map(|(d, m)| (d == best).then_some(m)).collect();
        candidates.choose(rng).cloned().cloned().unwrap_or(Move::Pass)
    }
}

/// Forward distance from `from` to `to` along `player`'s forward edges.
/// Returns `i64::MIN` when unreachable (so "max distance" sort correctly
/// ignores unreachable targets).
fn forward_steps(
    rules: &dyn Rules,
    from: SpaceId,
    to: SpaceId,
    player: crate::board::PlayerId,
) -> i64 {
    if from == to {
        return 0;
    }
    let mut cur = from;
    for steps in 1..=70i64 {
        match rules.forward_neighbor(cur, player) {
            Some(next) => {
                if next == to {
                    return steps;
                }
                cur = next;
            }
            None => return i64::MIN,
        }
    }
    i64::MIN
}
