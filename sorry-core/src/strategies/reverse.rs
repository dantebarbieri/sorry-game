use rand::RngCore;
use rand::seq::SliceRandom;

use crate::board::{PawnId, Space, SpaceId};
use crate::card::Card;
use crate::moves::Move;
use crate::rules::Rules;
use crate::strategy::{Complexity, Strategy, StrategyDescription, StrategyView};

use super::util::{greedy_pick, remaining_count};

/// "Reverse" — banks on a 4 or 10 to go "backward" and hit Home from the
/// near side of the track. Keeps a primary pawn hovering near
/// `start_exit`; secondary pawns stay in Start unless forced out. Once
/// the remaining 4s and 10s are exhausted it releases the primary forward
/// via Greedy.
#[derive(Debug, Default, Clone, Copy)]
pub struct ReverseStrategy;

impl Strategy for ReverseStrategy {
    fn name(&self) -> &str {
        "Reverse"
    }

    fn describe(&self) -> StrategyDescription {
        StrategyDescription {
            name: self.name().to_string(),
            summary: "Holds one pawn near start_exit and hopes for a 4 or chain of 10s."
                .to_string(),
            complexity: Complexity::Medium,
        }
    }

    fn choose_move(
        &self,
        view: &StrategyView,
        rules: &dyn Rules,
        card: Card,
        legal: &[Move],
        rng: &mut dyn RngCore,
    ) -> Move {
        let me = view.my_player;
        let start_area = rules.start_area(me);

        // Identify the primary: lowest-id pawn not in StartArea.
        let primary: Option<PawnId> = (0..rules.pawns_per_player() as u8)
            .map(PawnId)
            .find(|p| view.pawn_positions[me.0 as usize][p.0 as usize] != start_area);

        let hoard_remaining =
            remaining_count(view, rules, Card::Four) + remaining_count(view, rules, Card::Ten);

        match primary {
            None => {
                // No primary yet. If this card can start one, do so — we
                // need a pawn on the track to benefit from a 4 later.
                let starts: Vec<&Move> = legal
                    .iter()
                    .filter(|m| matches!(m, Move::StartPawn { .. }))
                    .collect();
                if let Some(m) = starts.choose(rng) {
                    return (*m).clone();
                }
                greedy_pick(view, rules, legal, rng)
            }
            Some(primary) => {
                // If we have a 4 or 10, commit the primary backward.
                if matches!(card, Card::Four | Card::Ten) {
                    let primary_retreats: Vec<&Move> = legal
                        .iter()
                        .filter(|m| matches!(m, Move::Retreat { pawn, .. } if *pawn == primary))
                        .collect();
                    if let Some(m) = primary_retreats.choose(rng) {
                        return (*m).clone();
                    }
                }
                // Non-4/10 card. Try to keep the primary near start_exit.
                // Filter out any move that advances the primary past a
                // "close to start_exit" window — unless no 4s/10s remain.
                if hoard_remaining > 0 {
                    let me_start_exit = rules.start_exit(me);
                    let primary_pos = view.pawn_positions[me.0 as usize][primary.0 as usize];
                    let primary_distance_from_exit =
                        distance_forward(rules, me_start_exit, primary_pos, me).unwrap_or(0);
                    // A "window" of 5 track squares around start_exit is
                    // the zone a backward-4 (equivalent to +15 forward)
                    // can still reach Home from cleanly.
                    let window = 5u32;
                    let allowed: Vec<&Move> = legal
                        .iter()
                        .filter(|m| match m {
                            Move::Advance { pawn, to, .. } if *pawn == primary => {
                                distance_forward(rules, me_start_exit, *to, me)
                                    .map(|d| d <= window)
                                    .unwrap_or(true)
                            }
                            Move::SplitSeven { first, second } => {
                                [first, second].iter().all(|leg| {
                                    if leg.pawn == primary {
                                        distance_forward(
                                            rules,
                                            me_start_exit,
                                            leg.to,
                                            me,
                                        )
                                        .map(|d| d <= window)
                                        .unwrap_or(true)
                                    } else {
                                        true
                                    }
                                })
                            }
                            _ => true,
                        })
                        .collect();
                    if !allowed.is_empty() {
                        let filtered: Vec<Move> = allowed.into_iter().cloned().collect();
                        let _ = primary_distance_from_exit; // debug-only aid
                        return greedy_pick(view, rules, &filtered, rng);
                    }
                }
                greedy_pick(view, rules, legal, rng)
            }
        }
    }
}

/// Forward steps from `from` to `to` along `player`'s forward edges, or
/// None if `to` is not reachable forward from `from` within 70 steps.
fn distance_forward(
    rules: &dyn Rules,
    from: SpaceId,
    to: SpaceId,
    player: crate::board::PlayerId,
) -> Option<u32> {
    if from == to {
        return Some(0);
    }
    if let Some(Space::StartArea(_)) = rules.classify(to) {
        return None;
    }
    let mut cur = from;
    for steps in 1..=70 {
        match rules.forward_neighbor(cur, player) {
            Some(next) => {
                if next == to {
                    return Some(steps);
                }
                cur = next;
            }
            None => return None,
        }
    }
    None
}
