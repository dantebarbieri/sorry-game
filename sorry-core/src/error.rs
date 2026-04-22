use serde::{Deserialize, Serialize};

use crate::board::{PawnId, PlayerId, SpaceId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SorryError {
    NotEnoughPlayers { min: usize, got: usize },
    TooManyPlayers { max: usize, got: usize },
    StrategyCountMismatch { expected: usize, got: usize },
    EmptyDeck,
    InvalidMove(String),
    InvalidPawn(PlayerId, PawnId),
    InvalidSpace(SpaceId),
    GameAlreadyOver,
    TurnLimitExceeded,
    /// A proposed move is not in the legal-move set for the current card.
    IllegalMove,
}

impl std::fmt::Display for SorryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotEnoughPlayers { min, got } => {
                write!(f, "need at least {min} players, got {got}")
            }
            Self::TooManyPlayers { max, got } => {
                write!(f, "at most {max} players allowed, got {got}")
            }
            Self::StrategyCountMismatch { expected, got } => {
                write!(f, "expected {expected} strategies, got {got}")
            }
            Self::EmptyDeck => write!(f, "deck is empty and reshuffle is disabled"),
            Self::InvalidMove(msg) => write!(f, "invalid move: {msg}"),
            Self::InvalidPawn(p, pw) => write!(f, "invalid pawn {pw:?} for player {p:?}"),
            Self::InvalidSpace(s) => write!(f, "invalid space {s:?}"),
            Self::GameAlreadyOver => write!(f, "game is already over"),
            Self::TurnLimitExceeded => write!(f, "turn limit exceeded"),
            Self::IllegalMove => write!(f, "move is not in the legal set"),
        }
    }
}

impl std::error::Error for SorryError {}

pub type Result<T> = std::result::Result<T, SorryError>;
