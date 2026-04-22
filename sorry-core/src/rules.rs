//! Rules trait and the standard Sorry! implementation.
//!
//! The board topology for `StandardRules` supports 4 player slots on a
//! 60-space outer ring. Pawn-per-player count and player-per-game count are
//! reported separately; a 2- or 3-player game reuses the same 4-slot board
//! with the extra slots simply unoccupied.

use rand::RngCore;
use rand::seq::SliceRandom;

use crate::board::{BoardState, PlayerId, Space, SpaceId};
use crate::card::{Card, standard_deck};

/// Resolved slide effect — the destination and every space the sliding pawn
/// traverses after leaving the slide head. The head itself is the `from`
/// parameter passed to [`Rules::slide_destination`] and is not included in
/// `traversed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlidePath {
    pub end: SpaceId,
    pub traversed: Vec<SpaceId>,
}

pub trait Rules: Send + Sync {
    fn name(&self) -> &str;

    fn min_players(&self) -> usize {
        2
    }
    fn max_players(&self) -> usize {
        4
    }
    fn pawns_per_player(&self) -> usize {
        4
    }
    /// 0 means "draw and play immediately with no persistent hand".
    fn hand_size(&self) -> usize {
        0
    }

    fn build_deck(&self) -> Vec<Card>;
    fn reshuffle_on_empty_deck(&self) -> bool {
        true
    }

    // Board graph — static topology, independent of BoardState.
    fn spaces(&self) -> &[Space];
    /// Classify a `SpaceId` as its semantic `Space`. Returns `None` for
    /// SpaceIds that aren't part of this rule's board.
    fn classify(&self, id: SpaceId) -> Option<Space>;
    fn forward_neighbor(&self, from: SpaceId, player: PlayerId) -> Option<SpaceId>;
    fn backward_neighbor(&self, from: SpaceId, player: PlayerId) -> Option<SpaceId>;
    fn slide_destination(&self, from: SpaceId, player: PlayerId) -> Option<SlidePath>;
    fn start_area(&self, player: PlayerId) -> SpaceId;
    fn start_exit(&self, player: PlayerId) -> SpaceId;
    fn home(&self, player: PlayerId) -> SpaceId;

    // Card semantics (per-card, so variants can remap).
    fn forward_steps(&self, card: Card) -> Option<i8>;
    fn backward_steps(&self, card: Card) -> Option<i8>;
    fn can_split(&self, card: Card) -> bool;
    fn split_total(&self, card: Card) -> Option<u8>;
    fn is_sorry(&self, card: Card) -> bool;
    fn is_swap(&self, card: Card) -> bool;
    fn can_start_pawn(&self, card: Card) -> bool;
    fn grants_extra_turn(&self, card: Card) -> bool;

    fn starting_player(&self, num_players: usize, rng: &mut dyn RngCore) -> PlayerId;
    fn is_winner(&self, board: &BoardState, player: PlayerId) -> bool;
    fn resolve_winners(&self, board: &BoardState, num_players: usize) -> Vec<PlayerId>;
}

// ─── StandardRules ──────────────────────────────────────────────────────
//
// SpaceId layout (4 player slots):
//   Track(i) for i in 0..60           → SpaceId(i)
//   StartArea(p) for p in 0..4        → SpaceId(60 + p)
//   Safety(p, i) for p in 0..4, i<5   → SpaceId(64 + p*5 + i)
//   Home(p) for p in 0..4             → SpaceId(84 + p)
//
// Track layout:
//   Side s (s in 0..4) owns tracks s*15 .. s*15+15.
//   Player s's start-exit   = track s*15 + 4
//   Player s's safety-entry = track s*15 + 2   (forward from here enters safety[0])
//   Short slide on side s: head s*15+1 → end s*15+4 (4 spaces)
//   Long  slide on side s: head s*15+9 → end s*15+13 (5 spaces)
// Slide color equals the side's player index; own-color slides do not
// trigger.

const NUM_TRACK: u32 = 60;
const NUM_SIDES: u32 = 4;
const SIDE_LEN: u32 = NUM_TRACK / NUM_SIDES; // 15
const SAFETY_LEN: u8 = 5;

fn track_id(i: u32) -> SpaceId {
    SpaceId(i % NUM_TRACK)
}
fn start_area_id(p: PlayerId) -> SpaceId {
    SpaceId(NUM_TRACK + p.0 as u32)
}
fn safety_id(p: PlayerId, slot: u8) -> SpaceId {
    SpaceId(NUM_TRACK + NUM_SIDES + (p.0 as u32) * SAFETY_LEN as u32 + slot as u32)
}
fn home_id(p: PlayerId) -> SpaceId {
    SpaceId(NUM_TRACK + NUM_SIDES + NUM_SIDES * SAFETY_LEN as u32 + p.0 as u32)
}

fn classify(id: SpaceId) -> Option<Space> {
    let v = id.0;
    if v < NUM_TRACK {
        return Some(Space::Track(v));
    }
    let v = v - NUM_TRACK;
    if v < NUM_SIDES {
        return Some(Space::StartArea(PlayerId(v as u8)));
    }
    let v = v - NUM_SIDES;
    if v < NUM_SIDES * SAFETY_LEN as u32 {
        let p = (v / SAFETY_LEN as u32) as u8;
        let slot = (v % SAFETY_LEN as u32) as u8;
        return Some(Space::Safety(PlayerId(p), slot));
    }
    let v = v - NUM_SIDES * SAFETY_LEN as u32;
    if v < NUM_SIDES {
        return Some(Space::Home(PlayerId(v as u8)));
    }
    None
}

pub fn safety_entry_track(p: PlayerId) -> u32 {
    p.0 as u32 * SIDE_LEN + 2
}

pub struct StandardRules {
    spaces: Vec<Space>,
}

impl Default for StandardRules {
    fn default() -> Self {
        Self::new()
    }
}

impl StandardRules {
    pub fn new() -> Self {
        let mut spaces = Vec::with_capacity(88);
        for i in 0..NUM_TRACK {
            spaces.push(Space::Track(i));
        }
        for p in 0..NUM_SIDES as u8 {
            spaces.push(Space::StartArea(PlayerId(p)));
        }
        for p in 0..NUM_SIDES as u8 {
            for i in 0..SAFETY_LEN {
                spaces.push(Space::Safety(PlayerId(p), i));
            }
        }
        for p in 0..NUM_SIDES as u8 {
            spaces.push(Space::Home(PlayerId(p)));
        }
        Self { spaces }
    }

    pub fn classify_space(&self, id: SpaceId) -> Option<Space> {
        classify(id)
    }

    /// The `SpaceId` of the `i`-th outer-track space. Panics if `i >= 60`.
    pub fn track_space(&self, i: u32) -> SpaceId {
        assert!(i < NUM_TRACK, "track index {i} out of range");
        track_id(i)
    }

    /// The `SpaceId` of the `slot`-th space in `player`'s safety zone.
    /// Panics if `slot >= 5`.
    pub fn safety_space(&self, player: PlayerId, slot: u8) -> SpaceId {
        assert!(slot < SAFETY_LEN, "safety slot {slot} out of range");
        safety_id(player, slot)
    }
}

impl Rules for StandardRules {
    fn name(&self) -> &str {
        "Standard"
    }

    fn build_deck(&self) -> Vec<Card> {
        standard_deck()
    }

    fn spaces(&self) -> &[Space] {
        &self.spaces
    }

    fn classify(&self, id: SpaceId) -> Option<Space> {
        classify(id)
    }

    fn forward_neighbor(&self, from: SpaceId, player: PlayerId) -> Option<SpaceId> {
        match classify(from)? {
            Space::Track(i) => {
                if i == safety_entry_track(player) {
                    Some(safety_id(player, 0))
                } else {
                    Some(track_id(i + 1))
                }
            }
            Space::Safety(owner, slot) => {
                if owner != player {
                    return None;
                }
                if slot + 1 < SAFETY_LEN {
                    Some(safety_id(player, slot + 1))
                } else {
                    Some(home_id(player))
                }
            }
            Space::StartArea(_) | Space::Home(_) => None,
        }
    }

    fn backward_neighbor(&self, from: SpaceId, player: PlayerId) -> Option<SpaceId> {
        match classify(from)? {
            Space::Track(i) => {
                let prev = (i + NUM_TRACK - 1) % NUM_TRACK;
                Some(track_id(prev))
            }
            Space::Safety(owner, slot) => {
                if owner != player {
                    return None;
                }
                if slot == 0 {
                    Some(track_id(safety_entry_track(player)))
                } else {
                    Some(safety_id(player, slot - 1))
                }
            }
            Space::StartArea(_) | Space::Home(_) => None,
        }
    }

    fn slide_destination(&self, from: SpaceId, player: PlayerId) -> Option<SlidePath> {
        let Space::Track(i) = classify(from)? else {
            return None;
        };
        let side = i / SIDE_LEN;
        let pos_in_side = i % SIDE_LEN;
        if side as u8 == player.0 {
            return None; // own-color slide does not trigger
        }
        let base = side * SIDE_LEN;
        match pos_in_side {
            1 => {
                // Short slide: head s*15+1 → end s*15+4. Traversed = +2, +3, +4.
                let traversed = (2..=4).map(|d| track_id(base + d)).collect();
                Some(SlidePath {
                    end: track_id(base + 4),
                    traversed,
                })
            }
            9 => {
                // Long slide: head s*15+9 → end s*15+13. Traversed = +10..+13.
                let traversed = (10..=13).map(|d| track_id(base + d)).collect();
                Some(SlidePath {
                    end: track_id(base + 13),
                    traversed,
                })
            }
            _ => None,
        }
    }

    fn start_area(&self, player: PlayerId) -> SpaceId {
        start_area_id(player)
    }
    fn start_exit(&self, player: PlayerId) -> SpaceId {
        track_id(player.0 as u32 * SIDE_LEN + 4)
    }
    fn home(&self, player: PlayerId) -> SpaceId {
        home_id(player)
    }

    fn forward_steps(&self, card: Card) -> Option<i8> {
        match card {
            Card::One => Some(1),
            Card::Two => Some(2),
            Card::Three => Some(3),
            Card::Five => Some(5),
            Card::Seven => Some(7),
            Card::Eight => Some(8),
            Card::Ten => Some(10),
            Card::Eleven => Some(11),
            Card::Twelve => Some(12),
            Card::Four | Card::Sorry => None,
        }
    }

    fn backward_steps(&self, card: Card) -> Option<i8> {
        match card {
            Card::Four => Some(4),
            Card::Ten => Some(1),
            _ => None,
        }
    }

    fn can_split(&self, card: Card) -> bool {
        matches!(card, Card::Seven)
    }
    fn split_total(&self, card: Card) -> Option<u8> {
        match card {
            Card::Seven => Some(7),
            _ => None,
        }
    }
    fn is_sorry(&self, card: Card) -> bool {
        matches!(card, Card::Sorry)
    }
    fn is_swap(&self, card: Card) -> bool {
        matches!(card, Card::Eleven)
    }
    fn can_start_pawn(&self, card: Card) -> bool {
        matches!(card, Card::One | Card::Two)
    }
    fn grants_extra_turn(&self, card: Card) -> bool {
        matches!(card, Card::Two)
    }

    fn starting_player(&self, num_players: usize, rng: &mut dyn RngCore) -> PlayerId {
        let mut ids: Vec<PlayerId> = (0..num_players as u8).map(PlayerId).collect();
        ids.shuffle(rng);
        ids[0]
    }

    fn is_winner(&self, board: &BoardState, player: PlayerId) -> bool {
        let home = home_id(player);
        board.pawns_of(player).iter().all(|s| *s == home)
    }

    fn resolve_winners(&self, board: &BoardState, num_players: usize) -> Vec<PlayerId> {
        (0..num_players as u8)
            .map(PlayerId)
            .filter(|p| self.is_winner(board, *p))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_count_is_88() {
        let r = StandardRules::new();
        assert_eq!(r.spaces().len(), 88);
    }

    #[test]
    fn classify_round_trips_all_spaces() {
        for i in 0..60 {
            assert_eq!(classify(track_id(i)), Some(Space::Track(i)));
        }
        for p in 0..4 {
            let pid = PlayerId(p);
            assert_eq!(classify(start_area_id(pid)), Some(Space::StartArea(pid)));
            assert_eq!(classify(home_id(pid)), Some(Space::Home(pid)));
            for i in 0..5 {
                assert_eq!(classify(safety_id(pid, i)), Some(Space::Safety(pid, i)));
            }
        }
    }

    #[test]
    fn forward_neighbor_track_wraps() {
        let r = StandardRules::new();
        let p = PlayerId(1);
        assert_eq!(r.forward_neighbor(track_id(59), p), Some(track_id(0)));
    }

    #[test]
    fn forward_neighbor_enters_own_safety() {
        let r = StandardRules::new();
        let p = PlayerId(0);
        // Player 0's safety entry = track 2.
        assert_eq!(r.forward_neighbor(track_id(2), p), Some(safety_id(p, 0)));
        // Other players continue on track.
        assert_eq!(
            r.forward_neighbor(track_id(2), PlayerId(1)),
            Some(track_id(3))
        );
    }

    #[test]
    fn safety_and_home_chain() {
        let r = StandardRules::new();
        let p = PlayerId(2);
        assert_eq!(
            r.forward_neighbor(safety_id(p, 0), p),
            Some(safety_id(p, 1))
        );
        assert_eq!(r.forward_neighbor(safety_id(p, 4), p), Some(home_id(p)));
        assert_eq!(r.forward_neighbor(home_id(p), p), None);
    }

    #[test]
    fn backward_from_safety_returns_to_track() {
        let r = StandardRules::new();
        let p = PlayerId(3);
        assert_eq!(
            r.backward_neighbor(safety_id(p, 0), p),
            Some(track_id(safety_entry_track(p)))
        );
    }

    #[test]
    fn own_color_slide_does_not_trigger() {
        let r = StandardRules::new();
        // Side 0 short slide head is track 1. Player 0's color matches side 0.
        assert!(r.slide_destination(track_id(1), PlayerId(0)).is_none());
    }

    #[test]
    fn other_color_slide_triggers() {
        let r = StandardRules::new();
        let path = r
            .slide_destination(track_id(1), PlayerId(1))
            .expect("slide should trigger");
        assert_eq!(path.end, track_id(4));
        assert_eq!(path.traversed, vec![track_id(2), track_id(3), track_id(4)]);
    }

    #[test]
    fn long_slide_traverses_five_spaces() {
        let r = StandardRules::new();
        let path = r
            .slide_destination(track_id(9), PlayerId(2))
            .expect("slide should trigger");
        assert_eq!(path.end, track_id(13));
        assert_eq!(path.traversed.len(), 4);
    }
}
