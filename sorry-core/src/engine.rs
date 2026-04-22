//! Shared draw/commit primitives used by both `Game` (simulator) and
//! `InteractiveGame`. Factored out so the two state machines produce
//! byte-identical `GameHistory` for the same seed.

use rand::rngs::StdRng;
use rand::seq::SliceRandom;

use crate::board::{BoardState, PlayerId};
use crate::card::Card;
use crate::error::{Result, SorryError};
use crate::history::{Action, TurnRecord};
use crate::moves::{Move, apply_move, legal_moves};
use crate::rules::Rules;

pub(crate) struct Engine<'a> {
    pub rules: &'a dyn Rules,
    pub board: &'a mut BoardState,
    pub rng: &'a mut StdRng,
    pub deck: &'a mut Vec<Card>,
    pub discard: &'a mut Vec<Card>,
}

impl<'a> Engine<'a> {
    pub fn draw_card(&mut self, turn: &mut TurnRecord) -> Result<Card> {
        if self.deck.is_empty() {
            if !self.rules.reshuffle_on_empty_deck() {
                return Err(SorryError::EmptyDeck);
            }
            if self.discard.is_empty() {
                return Err(SorryError::EmptyDeck);
            }
            self.deck.append(self.discard);
            self.deck.shuffle(self.rng);
            turn.actions.push(Action::Reshuffle);
        }
        self.deck.pop().ok_or(SorryError::EmptyDeck)
    }

    /// Validate `mv` is a legal move for `player` playing `card`, apply it to
    /// the board, discard the card, and append `Action::Play` to `turn`.
    pub fn commit_play(
        &mut self,
        player: PlayerId,
        card: Card,
        mv: &Move,
        turn: &mut TurnRecord,
    ) -> Result<()> {
        let legal = legal_moves(self.rules, self.board, player, card);
        if !legal.contains(mv) {
            return Err(SorryError::IllegalMove);
        }
        let (bumps, slides) = apply_move(self.rules, self.board, player, mv);
        self.discard.push(card);
        turn.actions.push(Action::Play {
            card,
            mv: mv.clone(),
            bumps,
            slides,
        });
        Ok(())
    }
}
