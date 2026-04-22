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
use crate::geometry::{BoardGeometry, PlayerLayout, SlideLayout, SpaceLayout};

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

    /// Publish a normalized `[-1, 1]` layout for every `SpaceId` on this
    /// board, plus per-player adjacency and slide data. One-shot snapshot —
    /// immutable for the life of a game; consumed by the renderer.
    fn board_geometry(&self) -> BoardGeometry;

    // Card semantics (per-card, so variants can remap).
    fn forward_steps(&self, card: Card) -> Option<i8>;
    fn backward_steps(&self, card: Card) -> Option<i8>;
    fn can_split(&self, card: Card) -> bool;
    fn split_total(&self, card: Card) -> Option<u8>;
    fn is_sorry(&self, card: Card) -> bool;
    fn is_swap(&self, card: Card) -> bool;
    /// Extra forward steps a pawn takes after exiting Start when this card
    /// is played. `None` means the card cannot start a pawn; `Some(0)`
    /// means the pawn lands on `start_exit`; `Some(n)` advances `n` spaces
    /// past `start_exit`. Every space on the path must be clear of the
    /// player's own pawns for the start to be legal.
    fn start_pawn_advance(&self, card: Card) -> Option<u32>;
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

    fn board_geometry(&self) -> BoardGeometry {
        // Square board: tracks run along the perimeter of a [-0.9, 0.9] box
        // inside the [-1, 1] layout square. Safety zones run inward from each
        // side's safety entry (track s*15+2). Each side's colors belong to
        // `PlayerId(s)`.
        //
        //   Side 0: bottom edge, moving +x (tangent 0°)
        //   Side 1: right  edge, moving +y (tangent 90°)
        //   Side 2: top    edge, moving -x (tangent 180°)
        //   Side 3: left   edge, moving -y (tangent 270°)
        struct SideGeom {
            corner: (f32, f32), // start corner where this side's track begins
            edge: (f32, f32),   // unit vector along the edge, direction of travel
            inward: (f32, f32), // unit vector perpendicular to edge, toward board center
            tangent_deg: f32,
        }
        let sides: [SideGeom; 4] = [
            SideGeom {
                corner: (-0.9, -0.9),
                edge: (1.0, 0.0),
                inward: (0.0, 1.0),
                tangent_deg: 0.0,
            },
            SideGeom {
                corner: (0.9, -0.9),
                edge: (0.0, 1.0),
                inward: (-1.0, 0.0),
                tangent_deg: 90.0,
            },
            SideGeom {
                corner: (0.9, 0.9),
                edge: (-1.0, 0.0),
                inward: (0.0, -1.0),
                tangent_deg: 180.0,
            },
            SideGeom {
                corner: (-0.9, 0.9),
                edge: (0.0, -1.0),
                inward: (1.0, 0.0),
                tangent_deg: 270.0,
            },
        ];
        let edge_len: f32 = 1.8;
        let safety_step: f32 = 0.09;
        let safety_first_depth: f32 = 0.13;
        let start_depth: f32 = 0.22;
        // Start sits directly inward from `start_exit` (track `s*15+4`), so
        // a pawn leaving Start is perpendicular to its exit tile — matches
        // the physical Sorry! board's Start pocket layout.
        let start_t: f32 = 4.0 / SIDE_LEN as f32;

        // Local fraction along a side for the `i`-th (0-indexed) track
        // space. Using `i / SIDE_LEN` places the 15 spaces at 0..14/15 of
        // the edge, so the *first* tile of side `s` sits exactly at side
        // `s`'s starting corner — the corner where side `s-1` hands off.
        // That corner tile doubles as the visual board corner and keeps the
        // short/long slide heads (track_s*15+1, +9) a full tile further from
        // the corner than they would be with the last-tile-at-corner layout.
        let track_local_t = |i: u32| -> f32 { i as f32 / SIDE_LEN as f32 };
        let track_center = |s: usize, i: u32| -> [f32; 2] {
            let side = &sides[s];
            let t = track_local_t(i);
            [
                side.corner.0 + side.edge.0 * t * edge_len,
                side.corner.1 + side.edge.1 * t * edge_len,
            ]
        };

        let adjacency = |id: SpaceId| -> (Vec<Option<SpaceId>>, Vec<Option<SpaceId>>) {
            let mut fwd = Vec::with_capacity(NUM_SIDES as usize);
            let mut bwd = Vec::with_capacity(NUM_SIDES as usize);
            for p in 0..NUM_SIDES as u8 {
                fwd.push(self.forward_neighbor(id, PlayerId(p)));
                bwd.push(self.backward_neighbor(id, PlayerId(p)));
            }
            (fwd, bwd)
        };

        let mut spaces: Vec<SpaceLayout> = Vec::with_capacity(88);

        // Tracks (0..60) — iterate in SpaceId order so `spaces` matches `spaces()`.
        for (s, side) in sides.iter().enumerate() {
            for i in 0..SIDE_LEN {
                let id = track_id(s as u32 * SIDE_LEN + i);
                let (forward, backward) = adjacency(id);
                let t = track_local_t(i);
                spaces.push(SpaceLayout {
                    id,
                    kind: Space::Track(s as u32 * SIDE_LEN + i),
                    center: [
                        side.corner.0 + side.edge.0 * t * edge_len,
                        side.corner.1 + side.edge.1 * t * edge_len,
                    ],
                    tangent_deg: side.tangent_deg,
                    forward,
                    backward,
                });
            }
        }

        // Start areas (60..64)
        for s in 0..NUM_SIDES as u8 {
            let side = &sides[s as usize];
            let cx = side.corner.0 + side.edge.0 * start_t * edge_len + side.inward.0 * start_depth;
            let cy = side.corner.1 + side.edge.1 * start_t * edge_len + side.inward.1 * start_depth;
            let id = start_area_id(PlayerId(s));
            let (forward, backward) = adjacency(id);
            spaces.push(SpaceLayout {
                id,
                kind: Space::StartArea(PlayerId(s)),
                center: [cx, cy],
                tangent_deg: sides[s as usize].tangent_deg,
                forward,
                backward,
            });
        }

        // Safety zones (64..84) — 5 slots per player, inward from track s*15+2.
        for s in 0..NUM_SIDES as u8 {
            let side = &sides[s as usize];
            let entry = track_center(s as usize, 2);
            let inward_tangent = (sides[s as usize].tangent_deg + 90.0) % 360.0;
            for slot in 0..SAFETY_LEN {
                let depth = safety_first_depth + (slot as f32) * safety_step;
                let cx = entry[0] + side.inward.0 * depth;
                let cy = entry[1] + side.inward.1 * depth;
                let id = safety_id(PlayerId(s), slot);
                let (forward, backward) = adjacency(id);
                spaces.push(SpaceLayout {
                    id,
                    kind: Space::Safety(PlayerId(s), slot),
                    center: [cx, cy],
                    tangent_deg: inward_tangent,
                    forward,
                    backward,
                });
            }
        }

        // Homes (84..88) — one step further inward from safety slot 4.
        for s in 0..NUM_SIDES as u8 {
            let side = &sides[s as usize];
            let entry = track_center(s as usize, 2);
            let depth = safety_first_depth + (SAFETY_LEN as f32) * safety_step + 0.05;
            let cx = entry[0] + side.inward.0 * depth;
            let cy = entry[1] + side.inward.1 * depth;
            let id = home_id(PlayerId(s));
            let (forward, backward) = adjacency(id);
            spaces.push(SpaceLayout {
                id,
                kind: Space::Home(PlayerId(s)),
                center: [cx, cy],
                tangent_deg: (sides[s as usize].tangent_deg + 90.0) % 360.0,
                forward,
                backward,
            });
        }

        // Slides — short (head +1) and long (head +9) per side. Query
        // slide_destination from any non-owner to extract the path.
        let mut slides: Vec<SlideLayout> = Vec::with_capacity(8);
        for s in 0..NUM_SIDES as u8 {
            let owner = PlayerId(s);
            let victim = PlayerId((s + 1) % NUM_SIDES as u8);
            for head_offset in [1u32, 9u32] {
                let head = track_id(s as u32 * SIDE_LEN + head_offset);
                if let Some(path) = self.slide_destination(head, victim) {
                    slides.push(SlideLayout {
                        head,
                        end: path.end,
                        path: path.traversed,
                        owner,
                    });
                }
            }
        }

        let players = (0..NUM_SIDES as u8)
            .map(|s| PlayerLayout {
                player: PlayerId(s),
                start_area: start_area_id(PlayerId(s)),
                start_exit: track_id(s as u32 * SIDE_LEN + 4),
                home: home_id(PlayerId(s)),
            })
            .collect();

        BoardGeometry {
            bounds: [-1.0, -1.0, 1.0, 1.0],
            spaces,
            slides,
            players,
        }
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
    fn start_pawn_advance(&self, card: Card) -> Option<u32> {
        match card {
            // A 1 drops the pawn on start_exit.
            Card::One => Some(0),
            // A 2 drops the pawn on start_exit and then one more space
            // forward. Classic Sorry! rule — and the 2 also grants an
            // extra turn, handled by `grants_extra_turn`.
            Card::Two => Some(1),
            _ => None,
        }
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

    #[test]
    fn board_geometry_has_88_spaces_in_canonical_order() {
        let r = StandardRules::new();
        let g = r.board_geometry();
        assert_eq!(g.spaces.len(), 88);
        // Tracks first, then StartAreas, then Safeties, then Homes — matches
        // the ordering of `spaces()`.
        for i in 0..60 {
            assert_eq!(g.spaces[i].id, track_id(i as u32));
            assert!(matches!(g.spaces[i].kind, Space::Track(_)));
        }
        for p in 0..4 {
            assert!(matches!(g.spaces[60 + p].kind, Space::StartArea(_)));
        }
        for i in 64..84 {
            assert!(matches!(g.spaces[i].kind, Space::Safety(_, _)));
        }
        for p in 0..4 {
            assert!(matches!(g.spaces[84 + p].kind, Space::Home(_)));
        }
    }

    #[test]
    fn board_geometry_slide_count_is_eight() {
        let r = StandardRules::new();
        assert_eq!(r.board_geometry().slides.len(), 8);
    }

    #[test]
    fn board_geometry_players_published() {
        let r = StandardRules::new();
        let g = r.board_geometry();
        assert_eq!(g.players.len(), 4);
        for s in 0..4u8 {
            let p = &g.players[s as usize];
            assert_eq!(p.player, PlayerId(s));
            assert_eq!(p.start_area, start_area_id(PlayerId(s)));
            assert_eq!(p.home, home_id(PlayerId(s)));
            assert_eq!(p.start_exit, track_id(s as u32 * SIDE_LEN + 4));
        }
    }

    #[test]
    fn board_geometry_adjacency_matches_trait() {
        let r = StandardRules::new();
        let g = r.board_geometry();
        // Per-space adjacency vectors are indexed by PlayerId.0 and must
        // match `forward_neighbor` / `backward_neighbor` called directly.
        for layout in &g.spaces {
            for p in 0..4u8 {
                assert_eq!(
                    layout.forward[p as usize],
                    r.forward_neighbor(layout.id, PlayerId(p)),
                    "forward mismatch at space {:?} for player {p}",
                    layout.kind
                );
                assert_eq!(
                    layout.backward[p as usize],
                    r.backward_neighbor(layout.id, PlayerId(p)),
                    "backward mismatch at space {:?} for player {p}",
                    layout.kind
                );
            }
        }
    }

    #[test]
    fn board_geometry_track_centers_on_square_perimeter() {
        let r = StandardRules::new();
        let g = r.board_geometry();
        // Side 0 (bottom): y = -0.9, x increasing.
        let s0_track0 = &g.spaces[0];
        assert!((s0_track0.center[1] - -0.9).abs() < 1e-5);
        assert!(s0_track0.center[0] < 0.0);
        assert!((s0_track0.tangent_deg - 0.0).abs() < 1e-5);
        // Side 2 (top): y = +0.9, space index 30.
        let s2_track0 = &g.spaces[30];
        assert!((s2_track0.center[1] - 0.9).abs() < 1e-5);
        assert!((s2_track0.tangent_deg - 180.0).abs() < 1e-5);
    }

    #[test]
    fn board_geometry_bounds_are_unit_square() {
        let r = StandardRules::new();
        let g = r.board_geometry();
        assert_eq!(g.bounds, [-1.0, -1.0, 1.0, 1.0]);
        // Every space center lies within bounds.
        for layout in &g.spaces {
            assert!(layout.center[0] >= -1.0 && layout.center[0] <= 1.0);
            assert!(layout.center[1] >= -1.0 && layout.center[1] <= 1.0);
        }
    }

    #[test]
    fn board_geometry_json_shape_is_frontend_friendly() {
        // This test pins the serialized JSON shape so the TS mirror types in
        // `frontend/src/lib/board/geometry.ts` stay in sync. Newtype structs
        // (`SpaceId`, `PlayerId`) serialize transparently as their inner value.
        let r = StandardRules::new();
        let g = r.board_geometry();
        let v = serde_json::to_value(&g).unwrap();
        // Top-level keys
        assert!(v.get("bounds").is_some());
        assert!(v.get("spaces").is_some());
        assert!(v.get("slides").is_some());
        assert!(v.get("players").is_some());
        // SpaceId is transparent: space.id is a bare number.
        let s0 = &v["spaces"][0];
        assert!(s0["id"].is_number(), "SpaceId should serialize as a number");
        // Space enum uses default serde externally-tagged form: {"Track": 0} etc.
        assert!(s0["kind"]["Track"].is_number());
        // center is [x, y]
        assert_eq!(s0["center"].as_array().unwrap().len(), 2);
        // forward/backward are arrays of length 4 (one per player slot).
        assert_eq!(s0["forward"].as_array().unwrap().len(), 4);
        assert_eq!(s0["backward"].as_array().unwrap().len(), 4);
        // PlayerLayout also uses transparent newtype serialization.
        let p0 = &v["players"][0];
        assert!(p0["player"].is_number());
        assert!(p0["home"].is_number());
        // Slide path is array of bare SpaceId numbers.
        let slide0 = &v["slides"][0];
        assert!(slide0["path"].as_array().unwrap()[0].is_number());
        assert!(slide0["owner"].is_number());
    }

    #[test]
    fn board_geometry_safety_chain_moves_inward() {
        let r = StandardRules::new();
        let g = r.board_geometry();
        // Player 0 safety is on side 0 (bottom), inward = +y. Slot 4 must be
        // strictly closer to board center (higher y) than slot 0.
        let p = PlayerId(0);
        let find = |s: SpaceId| g.spaces.iter().find(|l| l.id == s).unwrap();
        let s0 = find(safety_id(p, 0)).center[1];
        let s4 = find(safety_id(p, 4)).center[1];
        let home = find(home_id(p)).center[1];
        assert!(s4 > s0);
        assert!(home > s4);
    }
}
