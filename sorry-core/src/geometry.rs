//! Board geometry published by `Rules`. Single source of truth for where each
//! `SpaceId` lives in normalized `[-1, 1]` board-local coordinates, plus the
//! per-player adjacency graph a client walks for hop-by-hop animation without
//! crossing the WASM boundary.

use serde::{Deserialize, Serialize};

use crate::board::{PlayerId, Space, SpaceId};

/// Layout for a single space. `center` is in the board-local `[-1, 1]` square.
/// `tangent_deg` is the direction a pawn on this space faces, measured
/// counter-clockwise from `+x` in degrees.
///
/// `forward[i]` / `backward[i]` is the per-player adjacency for `PlayerId(i)`.
/// `None` means the neighbor is off the board for that player (e.g. `Home`
/// has no forward neighbor; another player's `Safety` is unreachable).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpaceLayout {
    pub id: SpaceId,
    pub kind: Space,
    pub center: [f32; 2],
    pub tangent_deg: f32,
    pub forward: Vec<Option<SpaceId>>,
    pub backward: Vec<Option<SpaceId>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlideLayout {
    pub head: SpaceId,
    pub end: SpaceId,
    pub path: Vec<SpaceId>,
    pub owner: PlayerId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerLayout {
    pub player: PlayerId,
    pub start_area: SpaceId,
    pub start_exit: SpaceId,
    pub home: SpaceId,
}

/// Full board layout snapshot — static per `Rules` impl, immutable for the
/// life of a game. `bounds` is `[xmin, ymin, xmax, ymax]` of the usable
/// layout region.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoardGeometry {
    pub bounds: [f32; 4],
    pub spaces: Vec<SpaceLayout>,
    pub slides: Vec<SlideLayout>,
    pub players: Vec<PlayerLayout>,
}
