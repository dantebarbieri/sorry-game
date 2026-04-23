use rand::RngCore;

use crate::card::Card;
use crate::moves::Move;
use crate::rules::Rules;
use crate::strategy::{Complexity, Strategy, StrategyDescription, StrategyView};

use super::util::greedy_pick;

/// "Not Sorry" — maximizes pawns out of Start. If any legal move starts a
/// pawn (card 1/2 StartPawn, or Sorry from Start), take it. Otherwise
/// fall through to Greedy.
#[derive(Debug, Default, Clone, Copy)]
pub struct NotSorryStrategy;

impl Strategy for NotSorryStrategy {
    fn name(&self) -> &str {
        "Not Sorry"
    }

    fn describe(&self) -> StrategyDescription {
        StrategyDescription {
            name: self.name().to_string(),
            summary: "Prioritizes starting pawns off their Start square; Greedy otherwise."
                .to_string(),
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
        let start_moves: Vec<&Move> = legal
            .iter()
            .filter(|m| matches!(m, Move::StartPawn { .. } | Move::Sorry { .. }))
            .collect();
        if !start_moves.is_empty() {
            // Maximize distance travelled by the started pawn. Sorry sends
            // it wherever the opponent was (often mid-track); card-2
            // StartPawn lands one past start_exit; card-1 lands on
            // start_exit itself. Measure by walking backward from the
            // landing square to start_exit and counting steps.
            let start_exit = rules.start_exit(me);
            let distance_from_exit = |to: crate::board::SpaceId| -> i32 {
                let mut cur = to;
                let mut dist: i32 = 0;
                while cur != start_exit {
                    match rules.backward_neighbor(cur, me) {
                        Some(prev) => cur = prev,
                        None => return -1,
                    }
                    dist += 1;
                    if dist > 70 {
                        return 70;
                    }
                }
                dist
            };
            let scored: Vec<(i32, &Move)> = start_moves
                .iter()
                .map(|m| {
                    let to = match m {
                        Move::StartPawn { to, .. } | Move::Sorry { to, .. } => *to,
                        _ => unreachable!(),
                    };
                    (distance_from_exit(to), *m)
                })
                .collect();
            let best = scored.iter().map(|(d, _)| *d).max().unwrap_or(-1);
            let candidates: Vec<&Move> =
                scored.into_iter().filter_map(|(d, m)| (d == best).then_some(m)).collect();
            let idx = (rng.next_u32() as usize) % candidates.len();
            return candidates[idx].clone();
        }
        greedy_pick(view, rules, legal, rng)
    }
}
