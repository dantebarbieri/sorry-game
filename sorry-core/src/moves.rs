//! Move enumeration, legal-move generation, and move application.
//!
//! Legal-move generation enforces the "cannot land on your own pawn" rule at
//! the step-count destination. Slide-triggered bumps (including own-pawn
//! bumps) are a *consequence* of a legal move and happen during
//! [`apply_move`], not during [`legal_moves`] filtering.

use serde::{Deserialize, Serialize};

use crate::board::{BoardState, PawnId, PlayerId, Space, SpaceId};
use crate::card::Card;
use crate::rules::Rules;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Move {
    /// Move a pawn forward by `card_value` steps. `to` is the step-count
    /// destination before any slide resolution.
    Advance {
        pawn: PawnId,
        card_value: i8,
        to: SpaceId,
    },
    /// Move a pawn backward (4, or 10-as-back-1).
    Retreat {
        pawn: PawnId,
        card_value: i8,
        to: SpaceId,
    },
    /// Split a 7 across two distinct pawns. Each half is resolved
    /// independently and must be individually legal. Both halves are
    /// non-zero and sum to 7.
    SplitSeven { first: SplitLeg, second: SplitLeg },
    /// 11 as a swap with an opponent pawn on the outer track.
    SwapEleven {
        my_pawn: PawnId,
        their_player: PlayerId,
        their_pawn: PawnId,
    },
    /// Start a pawn (1 or 2) from the player's StartArea onto their
    /// start-exit track space.
    StartPawn { pawn: PawnId, to: SpaceId },
    /// Sorry card — move a pawn from StartArea onto an opponent's track
    /// space, bumping that opponent to their StartArea.
    Sorry {
        my_pawn: PawnId,
        their_player: PlayerId,
        their_pawn: PawnId,
        to: SpaceId,
    },
    /// No legal move is available with the drawn card.
    Pass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitLeg {
    pub pawn: PawnId,
    pub steps: u8,
    pub to: SpaceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BumpEvent {
    pub player: PlayerId,
    pub pawn: PawnId,
    pub from: SpaceId,
    pub to: SpaceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlideEvent {
    pub player: PlayerId,
    pub pawn: PawnId,
    pub from: SpaceId,
    pub to: SpaceId,
    pub path: Vec<SpaceId>,
}

// ─── Legal moves ────────────────────────────────────────────────────────

pub fn legal_moves(
    rules: &dyn Rules,
    board: &BoardState,
    player: PlayerId,
    card: Card,
) -> Vec<Move> {
    let mut out = Vec::new();
    let num_pawns = rules.pawns_per_player();
    let start_area = rules.start_area(player);
    let start_exit = rules.start_exit(player);

    // StartPawn (1 or 2). `start_pawn_advance` tells us how many forward
    // steps past `start_exit` the pawn must travel on exit — 0 for a 1, 1
    // for a 2. Every space on the path (start_exit and any intermediate)
    // must be clear of the player's own pawns; own-pawn blockage here is
    // the "blocked off the circle" rule. Opponents aren't checked — they
    // get bumped at the landing space via `apply_move`.
    if let Some(extra_steps) = rules.start_pawn_advance(card) {
        let mut target = start_exit;
        let mut path_clear = !is_own_pawn_at(board, player, target);
        for _ in 0..extra_steps {
            if !path_clear {
                break;
            }
            match rules.forward_neighbor(target, player) {
                Some(next) => {
                    target = next;
                    if is_own_pawn_at(board, player, target) {
                        path_clear = false;
                    }
                }
                None => path_clear = false,
            }
        }
        if path_clear {
            for pw in 0..num_pawns as u8 {
                let pawn = PawnId(pw);
                if board.position(player, pawn) == start_area {
                    out.push(Move::StartPawn { pawn, to: target });
                    break; // all Start pawns are interchangeable; one canonical move
                }
            }
        }
    }

    // Advance (forward N).
    if let Some(n) = rules.forward_steps(card) {
        for pw in 0..num_pawns as u8 {
            let pawn = PawnId(pw);
            let from = board.position(player, pawn);
            if !can_step_from(rules, from, player) {
                continue;
            }
            if let Some(to) = walk_forward(rules, player, from, n)
                && is_landable(rules, board, player, to)
            {
                out.push(Move::Advance {
                    pawn,
                    card_value: n,
                    to,
                });
            }
        }
    }

    // Retreat (backward N).
    if let Some(n) = rules.backward_steps(card) {
        for pw in 0..num_pawns as u8 {
            let pawn = PawnId(pw);
            let from = board.position(player, pawn);
            if !can_step_from(rules, from, player) {
                continue;
            }
            if let Some(to) = walk_backward(rules, player, from, n)
                && is_landable(rules, board, player, to)
            {
                out.push(Move::Retreat {
                    pawn,
                    card_value: n,
                    to,
                });
            }
        }
    }

    // Split 7.
    if let (true, Some(total)) = (rules.can_split(card), rules.split_total(card)) {
        for a in 1..total {
            let b = total - a;
            if b == 0 {
                continue;
            }
            for pa in 0..num_pawns as u8 {
                for pb in 0..num_pawns as u8 {
                    if pa == pb {
                        continue;
                    }
                    let pawn_a = PawnId(pa);
                    let pawn_b = PawnId(pb);
                    let from_a = board.position(player, pawn_a);
                    let from_b = board.position(player, pawn_b);
                    if !can_step_from(rules, from_a, player)
                        || !can_step_from(rules, from_b, player)
                    {
                        continue;
                    }
                    let Some(to_a) = walk_forward(rules, player, from_a, a as i8) else {
                        continue;
                    };
                    let Some(to_b) = walk_forward(rules, player, from_b, b as i8) else {
                        continue;
                    };
                    if !is_landable(rules, board, player, to_a)
                        || !is_landable(rules, board, player, to_b)
                    {
                        continue;
                    }
                    if to_a == to_b
                        && !matches!(rules.classify(to_a), Some(Space::Home(_)))
                    {
                        // Two of your own pawns can't end up stacked on the
                        // same outer-track or safety space — but Home is
                        // a stacking space, so a 7 split as e.g. {1, 2}
                        // sending two safety pawns home together is fine.
                        continue;
                    }
                    out.push(Move::SplitSeven {
                        first: SplitLeg {
                            pawn: pawn_a,
                            steps: a,
                            to: to_a,
                        },
                        second: SplitLeg {
                            pawn: pawn_b,
                            steps: b,
                            to: to_b,
                        },
                    });
                }
            }
        }
    }

    // 11 swap.
    if rules.is_swap(card) {
        for pw in 0..num_pawns as u8 {
            let my_pawn = PawnId(pw);
            let from = board.position(player, my_pawn);
            if !is_on_outer_track(rules, from) {
                continue;
            }
            for other in 0..board.num_players() as u8 {
                if other == player.0 {
                    continue;
                }
                let their_player = PlayerId(other);
                for opw in 0..num_pawns as u8 {
                    let their_pawn = PawnId(opw);
                    let their_pos = board.position(their_player, their_pawn);
                    if !is_on_outer_track(rules, their_pos) {
                        continue;
                    }
                    out.push(Move::SwapEleven {
                        my_pawn,
                        their_player,
                        their_pawn,
                    });
                }
            }
        }
    }

    // Sorry.
    if rules.is_sorry(card) {
        // Need a pawn in our StartArea and an opponent pawn on the outer
        // track.
        let mut have_start_pawn: Option<PawnId> = None;
        for pw in 0..num_pawns as u8 {
            let pawn = PawnId(pw);
            if board.position(player, pawn) == start_area {
                have_start_pawn = Some(pawn);
                break;
            }
        }
        if let Some(my_pawn) = have_start_pawn {
            for other in 0..board.num_players() as u8 {
                if other == player.0 {
                    continue;
                }
                let their_player = PlayerId(other);
                for opw in 0..num_pawns as u8 {
                    let their_pawn = PawnId(opw);
                    let their_pos = board.position(their_player, their_pawn);
                    if is_on_outer_track(rules, their_pos) {
                        out.push(Move::Sorry {
                            my_pawn,
                            their_player,
                            their_pawn,
                            to: their_pos,
                        });
                    }
                }
            }
        }
    }

    // 11's "may" clause: if the player cannot move 11 forward, they are
    // not forced to swap — ending the turn is a legal alternative. This
    // matches the official Hasbro rule ("A player who cannot move 11
    // spaces is not forced to switch and instead can end their turn.")
    // and mirrors the common house rule. The 11 is the only card that
    // grants this election.
    if rules.is_swap(card) && !out.iter().any(|m| matches!(m, Move::Advance { .. })) {
        out.push(Move::Pass);
    }

    if out.is_empty() {
        out.push(Move::Pass);
    }
    out
}

fn can_step_from(rules: &dyn Rules, from: SpaceId, player: PlayerId) -> bool {
    match rules.classify(from) {
        Some(Space::Track(_)) => true,
        Some(Space::Safety(owner, _)) => owner == player,
        _ => false,
    }
}

fn is_on_outer_track(rules: &dyn Rules, space: SpaceId) -> bool {
    matches!(rules.classify(space), Some(Space::Track(_)))
}

fn is_own_pawn_at(board: &BoardState, player: PlayerId, space: SpaceId) -> bool {
    board.pawns_of(player).contains(&space)
}

/// Destination-legality check. A pawn may land on `to` unless an own
/// pawn already occupies that space — with the exception of `Home`,
/// where multiple of a player's pawns stack (that's how the game ends).
/// `StartArea` is technically a stacking space too, but no legal move
/// *lands* at Start, so this only has to special-case Home.
fn is_landable(rules: &dyn Rules, board: &BoardState, player: PlayerId, to: SpaceId) -> bool {
    if matches!(rules.classify(to), Some(Space::Home(_))) {
        return true;
    }
    !is_own_pawn_at(board, player, to)
}

fn walk_forward(rules: &dyn Rules, player: PlayerId, from: SpaceId, steps: i8) -> Option<SpaceId> {
    if steps <= 0 {
        return None;
    }
    let mut cur = from;
    for _ in 0..steps {
        cur = rules.forward_neighbor(cur, player)?;
    }
    Some(cur)
}

fn walk_backward(rules: &dyn Rules, player: PlayerId, from: SpaceId, steps: i8) -> Option<SpaceId> {
    if steps <= 0 {
        return None;
    }
    let mut cur = from;
    for _ in 0..steps {
        cur = rules.backward_neighbor(cur, player)?;
    }
    Some(cur)
}

// ─── Apply move ─────────────────────────────────────────────────────────

#[derive(Default)]
struct Effects {
    bumps: Vec<BumpEvent>,
    slides: Vec<SlideEvent>,
}

pub fn apply_move(
    rules: &dyn Rules,
    board: &mut BoardState,
    player: PlayerId,
    mv: &Move,
) -> (Vec<BumpEvent>, Vec<SlideEvent>) {
    let mut fx = Effects::default();
    match mv {
        Move::Pass => {}
        Move::Advance { pawn, to, .. }
        | Move::Retreat { pawn, to, .. }
        | Move::StartPawn { pawn, to } => {
            move_pawn(rules, board, player, *pawn, *to, true, &mut fx);
        }
        Move::SplitSeven { first, second } => {
            move_pawn(rules, board, player, first.pawn, first.to, true, &mut fx);
            move_pawn(rules, board, player, second.pawn, second.to, true, &mut fx);
        }
        Move::SwapEleven {
            my_pawn,
            their_player,
            their_pawn,
        } => {
            let my_space = board.position(player, *my_pawn);
            let their_space = board.position(*their_player, *their_pawn);
            board.set_position(player, *my_pawn, their_space);
            board.set_position(*their_player, *their_pawn, my_space);
            // Swap does not trigger slides (physical rule).
        }
        Move::Sorry {
            my_pawn,
            their_player,
            their_pawn,
            to,
        } => {
            let their_from = board.position(*their_player, *their_pawn);
            let their_start = rules.start_area(*their_player);
            board.set_position(*their_player, *their_pawn, their_start);
            fx.bumps.push(BumpEvent {
                player: *their_player,
                pawn: *their_pawn,
                from: their_from,
                to: their_start,
            });
            // Place our pawn. Slides can trigger from Sorry's landing; the
            // destination was just cleared above so no secondary bump there.
            move_pawn(rules, board, player, *my_pawn, *to, false, &mut fx);
        }
    }
    (fx.bumps, fx.slides)
}

fn move_pawn(
    rules: &dyn Rules,
    board: &mut BoardState,
    player: PlayerId,
    pawn: PawnId,
    to: SpaceId,
    bump_at_destination: bool,
    fx: &mut Effects,
) {
    // 1. Bump any opponent pawn parked at `to` (normal landing bump).
    if bump_at_destination {
        let parked: Vec<(PlayerId, PawnId)> = board
            .pawns_at(to)
            .into_iter()
            .filter(|(p, _)| *p != player)
            .collect();
        for (owner, their_pawn) in parked {
            let their_start = rules.start_area(owner);
            board.set_position(owner, their_pawn, their_start);
            fx.bumps.push(BumpEvent {
                player: owner,
                pawn: their_pawn,
                from: to,
                to: their_start,
            });
        }
    }
    // 2. Place player's pawn at `to`.
    board.set_position(player, pawn, to);
    // 3. Resolve slide if applicable.
    if let Some(path) = rules.slide_destination(to, player) {
        let final_space = path.end;
        // Collect all (player, pawn) pairs on any traversed space, except
        // the sliding pawn itself.
        let mut to_bump: Vec<(PlayerId, PawnId, SpaceId)> = Vec::new();
        for space in &path.traversed {
            for (owner, parked_pawn) in board.pawns_at(*space) {
                if owner == player && parked_pawn == pawn {
                    continue;
                }
                to_bump.push((owner, parked_pawn, *space));
            }
        }
        for (owner, parked_pawn, from) in to_bump {
            let owner_start = rules.start_area(owner);
            board.set_position(owner, parked_pawn, owner_start);
            fx.bumps.push(BumpEvent {
                player: owner,
                pawn: parked_pawn,
                from,
                to: owner_start,
            });
        }
        board.set_position(player, pawn, final_space);
        fx.slides.push(SlideEvent {
            player,
            pawn,
            from: to,
            to: final_space,
            path: path.traversed,
        });
    }
}
