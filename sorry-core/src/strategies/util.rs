//! Shared helpers used by multiple non-trivial strategies. Keep these
//! side-effect-free — strategies are called repeatedly per turn and the
//! engine re-derives state each time.

use rand::RngCore;
use rand::seq::SliceRandom;

use crate::board::{PawnId, PlayerId, Space, SpaceId};
use crate::card::Card;
use crate::moves::{Move, SplitLeg};
use crate::rules::Rules;
use crate::strategy::StrategyView;

/// Maximum walk length when counting steps to home. One full lap (60) plus
/// the 5-slot safety zone is a comfortable upper bound — anything beyond
/// that means the pawn can't reach home from `from` along forward edges,
/// and we return this as a "stuck" sentinel so such pawns sort as the
/// worst (for Greedy/Survivor) or the best (for Loser).
const STUCK_DISTANCE: u32 = 200;

/// StartArea pawns are "maximally far" from home for ranking purposes.
/// Picked to be strictly greater than any reachable forward distance
/// (~65) but below `STUCK_DISTANCE`, so ordering remains stable.
const START_DISTANCE: u32 = 100;

/// Forward-walk distance from `from` to `rules.home(player)` along that
/// player's forward edges. Returns `START_DISTANCE` when `from` is the
/// player's StartArea, `0` when `from` is already Home, and
/// `STUCK_DISTANCE` if Home is unreachable along forward edges (e.g., a
/// pawn parked in another player's safety — shouldn't happen in standard
/// play).
pub fn distance_to_home(rules: &dyn Rules, from: SpaceId, player: PlayerId) -> u32 {
    let home = rules.home(player);
    if from == home {
        return 0;
    }
    if let Some(Space::StartArea(owner)) = rules.classify(from)
        && owner == player
    {
        return START_DISTANCE;
    }
    let mut cur = from;
    for steps in 1..=STUCK_DISTANCE {
        match rules.forward_neighbor(cur, player) {
            Some(next) => {
                if next == home {
                    return steps;
                }
                cur = next;
            }
            None => return STUCK_DISTANCE,
        }
    }
    STUCK_DISTANCE
}

/// Sum of `distance_to_home` across every pawn of `player` in the view.
pub fn total_distance(view: &StrategyView, rules: &dyn Rules, player: PlayerId) -> u32 {
    view.pawn_positions[player.0 as usize]
        .iter()
        .map(|s| distance_to_home(rules, *s, player))
        .sum()
}

/// Scan `view.pawn_positions` for any non-`me` pawn sitting on `dest`.
pub fn bumps_opponent(
    view: &StrategyView,
    me: PlayerId,
    dest: SpaceId,
) -> Option<(PlayerId, PawnId)> {
    for (player_idx, pawns) in view.pawn_positions.iter().enumerate() {
        let player = PlayerId(player_idx as u8);
        if player == me {
            continue;
        }
        for (pawn_idx, space) in pawns.iter().enumerate() {
            if *space == dest {
                return Some((player, PawnId(pawn_idx as u8)));
            }
        }
    }
    None
}

/// Every (pawn, new-position) effect of applying `mv` — without actually
/// applying it. Used to simulate move outcomes for scoring.
///
/// - Advance / Retreat / StartPawn: one entry for the moving pawn.
/// - SplitSeven: two entries (both legs).
/// - SwapEleven: two entries (both swapped pawns' new positions).
/// - Sorry: one entry (the own pawn landing on the opponent's space).
///   The opponent going home-to-start is handled separately.
/// - Pass: empty.
pub fn own_destinations(mv: &Move, me: PlayerId) -> Vec<(PlayerId, PawnId, SpaceId)> {
    match mv {
        Move::Advance { pawn, to, .. }
        | Move::Retreat { pawn, to, .. }
        | Move::StartPawn { pawn, to } => vec![(me, *pawn, *to)],
        Move::SplitSeven { first, second } => vec![
            (me, first.pawn, first.to),
            (me, second.pawn, second.to),
        ],
        Move::SwapEleven {
            my_pawn,
            their_player,
            their_pawn,
        } => {
            // The swap's own-pawn destination is the opponent's current
            // position. The opponent goes to the own pawn's current pos —
            // that's in `opponent_destinations`.
            vec![(me, *my_pawn, SpaceId(u32::MAX)), (*their_player, *their_pawn, SpaceId(u32::MAX))]
        }
        Move::Sorry { my_pawn, to, .. } => vec![(me, *my_pawn, *to)],
        Move::Pass => Vec::new(),
    }
}

/// Produce post-move pawn positions for ALL players given the current
/// view and a candidate move. This is the core of "score a move" — we
/// deep-clone the position matrix and patch in the move's effects.
///
/// Slide effects are NOT simulated (too expensive to re-implement here);
/// the scorer works off the raw step-count destination, which is what
/// strategies choose against anyway. Real slides will be resolved by the
/// engine after `choose_move` returns.
pub fn simulate_positions(
    view: &StrategyView,
    me: PlayerId,
    mv: &Move,
) -> Vec<Vec<SpaceId>> {
    let mut pos = view.pawn_positions.clone();
    match mv {
        Move::Advance { pawn, to, .. }
        | Move::Retreat { pawn, to, .. }
        | Move::StartPawn { pawn, to } => {
            pos[me.0 as usize][pawn.0 as usize] = *to;
        }
        Move::SplitSeven { first, second } => {
            pos[me.0 as usize][first.pawn.0 as usize] = first.to;
            pos[me.0 as usize][second.pawn.0 as usize] = second.to;
        }
        Move::SwapEleven {
            my_pawn,
            their_player,
            their_pawn,
        } => {
            let my_pos = pos[me.0 as usize][my_pawn.0 as usize];
            let their_pos = pos[their_player.0 as usize][their_pawn.0 as usize];
            pos[me.0 as usize][my_pawn.0 as usize] = their_pos;
            pos[their_player.0 as usize][their_pawn.0 as usize] = my_pos;
        }
        Move::Sorry {
            my_pawn,
            their_player,
            their_pawn,
            to,
        } => {
            // Our pawn lands on `to`; the opponent pawn is bumped to their
            // Start area — approximated here with a sentinel the distance
            // helper will treat as StartArea (`their_player`'s start
            // classifies correctly through rules; callers pass rules when
            // they need distance-to-home).
            pos[me.0 as usize][my_pawn.0 as usize] = *to;
            pos[their_player.0 as usize][their_pawn.0 as usize] = SpaceId(u32::MAX);
        }
        Move::Pass => {}
    }
    pos
}

/// Total distance-to-home for `player` after simulating `mv`. Handles the
/// `SpaceId(u32::MAX)` sentinel produced by `simulate_positions` for a
/// bumped opponent pawn by treating it as StartArea (max distance).
pub fn simulated_distance(
    view: &StrategyView,
    rules: &dyn Rules,
    me: PlayerId,
    mv: &Move,
    for_player: PlayerId,
) -> u32 {
    let after = simulate_positions(view, me, mv);
    let start_area = rules.start_area(for_player);
    after[for_player.0 as usize]
        .iter()
        .map(|s| {
            if s.0 == u32::MAX {
                distance_to_home(rules, start_area, for_player)
            } else {
                distance_to_home(rules, *s, for_player)
            }
        })
        .sum()
}

/// Opponent with smallest mean distance-to-home. `None` if no opponents
/// exist (shouldn't happen — games require >= 2 players).
pub fn leader(view: &StrategyView, rules: &dyn Rules, me: PlayerId) -> Option<PlayerId> {
    let mut best: Option<(PlayerId, u64)> = None;
    for p_idx in 0..view.num_players {
        let p = PlayerId(p_idx as u8);
        if p == me {
            continue;
        }
        let sum: u64 = view.pawn_positions[p_idx]
            .iter()
            .map(|s| distance_to_home(rules, *s, p) as u64)
            .sum();
        match best {
            None => best = Some((p, sum)),
            Some((_, prev)) if sum < prev => best = Some((p, sum)),
            _ => {}
        }
    }
    best.map(|(p, _)| p)
}

/// Every destination square touched by `mv` (used for bump detection —
/// where would this move LAND?). Excludes Pass. For SwapEleven, returns
/// both swap endpoints. For Sorry, returns the landing square.
pub fn landing_squares(mv: &Move, view: &StrategyView, me: PlayerId) -> Vec<SpaceId> {
    match mv {
        Move::Advance { to, .. }
        | Move::Retreat { to, .. }
        | Move::StartPawn { to, .. }
        | Move::Sorry { to, .. } => vec![*to],
        Move::SplitSeven { first, second } => vec![first.to, second.to],
        Move::SwapEleven {
            my_pawn,
            their_player,
            their_pawn,
        } => {
            // Both endpoints — destination of our pawn (their old pos) and
            // destination of their pawn (our old pos).
            let my_pos = view.pawn_positions[me.0 as usize][my_pawn.0 as usize];
            let their_pos =
                view.pawn_positions[their_player.0 as usize][their_pawn.0 as usize];
            vec![their_pos, my_pos]
        }
        Move::Pass => Vec::new(),
    }
}

/// Does `mv` cause any pawn to be bumped to Start? True when a landing
/// square hits an opponent pawn (Advance/Retreat/StartPawn/Split-7 legs
/// all push onto a square; Sorry explicitly bumps). SwapEleven does NOT
/// bump — it relocates, it doesn't send anyone home.
pub fn is_bumping(mv: &Move, view: &StrategyView, me: PlayerId) -> bool {
    if matches!(mv, Move::Sorry { .. }) {
        return true;
    }
    if matches!(mv, Move::SwapEleven { .. } | Move::Pass) {
        return false;
    }
    landing_squares(mv, view, me)
        .iter()
        .any(|sq| bumps_opponent(view, me, *sq).is_some())
}

/// Greedy pick: minimize own total distance-to-home after the move.
/// Tiebreak: prefer moves whose primary destination is in the player's
/// safety zone. Final tiebreak: random among equals.
pub fn greedy_pick(
    view: &StrategyView,
    rules: &dyn Rules,
    legal: &[Move],
    rng: &mut dyn RngCore,
) -> Move {
    if legal.is_empty() {
        return Move::Pass;
    }
    let me = view.my_player;
    let scored: Vec<(u32, bool, &Move)> = legal
        .iter()
        .map(|m| {
            let d = simulated_distance(view, rules, me, m, me);
            let into_safety = matches!(m,
                Move::Advance { to, .. } | Move::StartPawn { to, .. } | Move::Sorry { to, .. }
                    if matches!(rules.classify(*to), Some(Space::Safety(owner, _)) if owner == me)
                        || *to == rules.home(me));
            (d, into_safety, m)
        })
        .collect();
    let best_dist = scored.iter().map(|(d, _, _)| *d).min().unwrap_or(u32::MAX);
    let tier1: Vec<_> = scored
        .iter()
        .filter(|(d, _, _)| *d == best_dist)
        .collect();
    let any_safety = tier1.iter().any(|(_, s, _)| *s);
    let tier2: Vec<&Move> = tier1
        .iter()
        .filter(|(_, s, _)| !any_safety || *s)
        .map(|(_, _, m)| *m)
        .collect();
    tier2.choose(rng).cloned().cloned().unwrap_or(Move::Pass)
}

/// Remaining copies of `card` in play — deck plus current player hands
/// minus cards already discarded. Used by Reverse for "are there still
/// 4s/10s left?" card counting. For `hand_size() == 0` rules, hands are
/// empty so this is exactly `total_in_deck - discarded`.
pub fn remaining_count(view: &StrategyView, rules: &dyn Rules, card: Card) -> u32 {
    let total = rules.build_deck().iter().filter(|c| **c == card).count() as u32;
    let discarded = view.discard.iter().filter(|c| **c == card).count() as u32;
    total.saturating_sub(discarded)
}

/// SplitLeg helper — the only place this module assembles legs itself.
/// Kept public so tests can construct expected moves.
pub fn leg(pawn: PawnId, steps: u8, to: SpaceId) -> SplitLeg {
    SplitLeg { pawn, steps, to }
}
