use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PlayerId(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PawnId(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SpaceId(pub u32);

/// A typed view of what kind of space a `SpaceId` refers to. Topology
/// (adjacency, slides, per-player mappings) lives in `Rules` — this enum
/// only classifies spaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Space {
    Track(u32),
    StartArea(PlayerId),
    Safety(PlayerId, u8),
    Home(PlayerId),
}

/// Mutable placement of every pawn in the game. Outer index is the player,
/// inner index is the pawn's id (0..pawns_per_player).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoardState {
    pawns: Vec<Vec<SpaceId>>,
}

impl BoardState {
    /// Fresh board with every pawn in its owner's Start area. Requires the
    /// caller to provide the SpaceId of each player's Start area (the Rules
    /// impl owns the mapping).
    pub fn fresh(num_players: usize, pawns_per_player: usize, start_area: &[SpaceId]) -> Self {
        assert_eq!(start_area.len(), num_players);
        let pawns = (0..num_players)
            .map(|p| vec![start_area[p]; pawns_per_player])
            .collect();
        Self { pawns }
    }

    pub fn num_players(&self) -> usize {
        self.pawns.len()
    }

    pub fn pawns_per_player(&self) -> usize {
        self.pawns.first().map(|v| v.len()).unwrap_or(0)
    }

    pub fn position(&self, player: PlayerId, pawn: PawnId) -> SpaceId {
        self.pawns[player.0 as usize][pawn.0 as usize]
    }

    pub fn pawns_of(&self, player: PlayerId) -> &[SpaceId] {
        &self.pawns[player.0 as usize]
    }

    /// Find which pawn (if any) currently occupies `space`. Returns the first
    /// match; callers should ensure only one pawn can occupy a given space at
    /// a time for track/safety spaces. Multiple pawns can stack in Start
    /// areas and Home — in that case the first is returned.
    pub fn pawn_at(&self, space: SpaceId) -> Option<(PlayerId, PawnId)> {
        for (p_idx, pawns) in self.pawns.iter().enumerate() {
            for (pw_idx, s) in pawns.iter().enumerate() {
                if *s == space {
                    return Some((PlayerId(p_idx as u8), PawnId(pw_idx as u8)));
                }
            }
        }
        None
    }

    /// All pawns currently on `space`, in (player, pawn) order.
    pub fn pawns_at(&self, space: SpaceId) -> Vec<(PlayerId, PawnId)> {
        let mut out = Vec::new();
        for (p_idx, pawns) in self.pawns.iter().enumerate() {
            for (pw_idx, s) in pawns.iter().enumerate() {
                if *s == space {
                    out.push((PlayerId(p_idx as u8), PawnId(pw_idx as u8)));
                }
            }
        }
        out
    }

    pub fn set_position(&mut self, player: PlayerId, pawn: PawnId, to: SpaceId) {
        self.pawns[player.0 as usize][pawn.0 as usize] = to;
    }

    /// Every pawn position, flattened as `positions[player][pawn] = space`.
    pub fn all_positions(&self) -> &[Vec<SpaceId>] {
        &self.pawns
    }
}
