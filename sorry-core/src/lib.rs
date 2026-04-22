//! Sorry! board game engine.
//!
//! The crate is organized around two trait objects that consumers pick at
//! runtime: `Box<dyn Rules>` (board geometry, deck, card semantics) and
//! `Box<dyn Strategy>` (how a player decides what to do). The `Game` struct
//! drives a round of play and emits a serializable `GameHistory` that is
//! sufficient to replay the game deterministically given the original seed.
//!
//! ### Hand-size convention
//!
//! `Rules::hand_size()` returns `0` for draw-and-play-immediately rule sets
//! (standard Sorry!) and `>= 1` for variants with a persistent hand. When
//! `hand_size() == 0` the engine skips `Strategy::choose_card` entirely and
//! passes the drawn card directly to `Strategy::choose_move`.

pub mod board;
pub mod card;
pub mod error;
pub mod game;
pub mod history;
pub mod moves;
pub mod rules;
pub mod strategies;
pub mod strategy;

pub use board::{BoardState, PawnId, PlayerId, Space, SpaceId};
pub use card::{Card, standard_deck};
pub use error::{Result, SorryError};
pub use game::Game;
pub use history::{Action, GameHistory, TurnRecord};
pub use moves::{BumpEvent, Move, SlideEvent, SplitLeg, apply_move, legal_moves};
pub use rules::{Rules, SlidePath, StandardRules};
pub use strategies::RandomStrategy;
pub use strategy::{Complexity, Strategy, StrategyDescription, StrategyView};
