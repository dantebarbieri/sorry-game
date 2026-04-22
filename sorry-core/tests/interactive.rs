//! Integration tests for the interactive-play API.
//!
//! The load-bearing test is
//! `interactive_history_matches_simulator_history_byte_for_byte` — it pins
//! the invariant that a fully-bot-driven `InteractiveGame` produces the same
//! `GameHistory` as the simulator `Game::play()` for the same seed and
//! strategies. If the two state machines ever drift, that test trips.

use sorry_core::{
    Action, ActionNeeded, Card, Game, InteractiveGame, Move, PawnId, PlayerAction, PlayerId, Rules,
    SorryError, SpaceId, StandardRules, Strategy,
};
use sorry_core::strategies::RandomStrategy;

fn random_strategies(n: usize) -> Vec<Box<dyn Strategy>> {
    (0..n)
        .map(|_| Box::new(RandomStrategy) as Box<dyn Strategy>)
        .collect()
}

fn interactive_game(num_players: usize, seed: u64) -> InteractiveGame {
    let rules: Box<dyn Rules> = Box::new(StandardRules::new());
    let names = vec!["Random".to_string(); num_players];
    InteractiveGame::new_with_strategy_names(rules, names, seed).unwrap()
}

fn drive_to_completion(game: &mut InteractiveGame) {
    let bot = RandomStrategy;
    while !game.is_over() {
        let a = game.get_bot_action(&bot).unwrap();
        game.apply_action(a).unwrap();
    }
}

#[test]
fn interactive_game_reaches_completion_via_apply_action() {
    let mut game = interactive_game(4, 1234);
    drive_to_completion(&mut game);
    assert!(game.is_over());
    let history = game.history();
    assert!(
        !history.winners.is_empty() || history.truncated,
        "expected either winners or a truncated game"
    );
    match game.action_needed() {
        ActionNeeded::GameOver { .. } => {}
        other => panic!("expected GameOver, got {other:?}"),
    }
}

#[test]
fn interactive_apply_action_is_deterministic_across_runs() {
    let mut a = interactive_game(4, 99);
    let mut b = interactive_game(4, 99);
    drive_to_completion(&mut a);
    drive_to_completion(&mut b);
    assert_eq!(a.history(), b.history());
}

#[test]
fn interactive_history_matches_simulator_history_byte_for_byte() {
    let seed = 1234;
    let num_players = 4;

    let rules: Box<dyn Rules> = Box::new(StandardRules::new());
    let sim_history = Game::new(rules, random_strategies(num_players), seed)
        .unwrap()
        .play()
        .unwrap();

    let mut game = interactive_game(num_players, seed);
    drive_to_completion(&mut game);

    assert_eq!(game.history(), &sim_history);
}

#[test]
fn apply_action_after_game_over_errors() {
    let mut game = interactive_game(4, 1234);
    drive_to_completion(&mut game);
    let err = game.apply_action(PlayerAction::PlayMove { mv: Move::Pass });
    assert!(matches!(err, Err(SorryError::GameAlreadyOver)));
}

#[test]
fn get_bot_action_after_game_over_errors() {
    let mut game = interactive_game(4, 1234);
    drive_to_completion(&mut game);
    let err = game.get_bot_action(&RandomStrategy);
    assert!(matches!(err, Err(SorryError::GameAlreadyOver)));
}

#[test]
fn apply_action_rejects_illegal_move() {
    let mut game = interactive_game(4, 42);
    let bogus = Move::Advance {
        pawn: PawnId(0),
        card_value: 99,
        to: SpaceId(999),
    };
    let err = game.apply_action(PlayerAction::PlayMove { mv: bogus });
    assert!(matches!(err, Err(SorryError::IllegalMove)));
}

#[test]
fn apply_action_rejects_unexpected_action_variant() {
    // Standard rules: action_needed is always ChooseMove (or GameOver).
    // Sending a ChooseCard should be rejected.
    let mut game = interactive_game(4, 42);
    match game.action_needed() {
        ActionNeeded::ChooseMove { .. } => {}
        other => panic!("expected ChooseMove at game start, got {other:?}"),
    }
    let err = game.apply_action(PlayerAction::ChooseCard { hand_index: 0 });
    assert!(matches!(
        err,
        Err(SorryError::UnexpectedAction {
            expected: "PlayMove",
            got: "ChooseCard"
        })
    ));
}

#[test]
fn two_grants_extra_turn_on_same_player() {
    // Drive a full game via the interactive API, tracking transitions. When
    // we apply a PlayMove for a Two that doesn't win, the new action_needed
    // should be ChooseMove for the SAME player (extra turn granted).
    let mut game = interactive_game(4, 1234);
    let bot = RandomStrategy;
    let mut saw_extra_turn_transition = false;
    while !game.is_over() {
        let before = game.action_needed().clone();
        let action = game.get_bot_action(&bot).unwrap();
        game.apply_action(action).unwrap();
        let after = game.action_needed().clone();
        if let (
            ActionNeeded::ChooseMove {
                player: p_before,
                card: Card::Two,
                ..
            },
            ActionNeeded::ChooseMove {
                player: p_after, ..
            },
        ) = (&before, &after)
            && p_before == p_after
        {
            saw_extra_turn_transition = true;
        }
    }
    // Across a full 4-player random game there will almost always be at
    // least one Two played mid-game (the deck has four Twos). If this ever
    // flakes, pick a different seed — but for seed 1234 the outcome is
    // deterministic.
    assert!(
        saw_extra_turn_transition,
        "expected at least one Two → same-player ChooseMove transition"
    );
    // Also verify the recorded TurnRecord has ExtraTurnGranted with a
    // subsequent Play.
    let history = game.history();
    let mut found_pattern = false;
    for turn in &history.turns {
        let mut seen_extra = false;
        for a in &turn.actions {
            if seen_extra && matches!(a, Action::Play { .. }) {
                found_pattern = true;
                break;
            }
            if matches!(a, Action::ExtraTurnGranted) {
                seen_extra = true;
            }
        }
        if found_pattern {
            break;
        }
    }
    assert!(
        found_pattern,
        "expected a TurnRecord containing ExtraTurnGranted followed by a Play"
    );
}

#[test]
fn pass_move_is_surfaced_and_applied() {
    // On turn 1 all pawns are in StartArea. Any card except 1 or 2 (and
    // Sorry! — which also needs an opponent on-track, not present at t=0)
    // leaves the player with no legal move → Pass.
    let mut seed = 0u64;
    let game = loop {
        let g = interactive_game(4, seed);
        if let ActionNeeded::ChooseMove { legal_moves, .. } = g.action_needed()
            && legal_moves.as_slice() == [Move::Pass]
        {
            break g;
        }
        seed += 1;
        assert!(seed < 1000, "could not find a Pass-only seed in 1000 tries");
    };
    let mut game = game;
    let starting_player = game.current_player();
    game.apply_action(PlayerAction::PlayMove { mv: Move::Pass })
        .unwrap();
    assert_ne!(
        game.current_player(),
        starting_player,
        "Pass should advance to the next player"
    );
}

#[test]
fn player_view_hides_drawn_card_from_spectators() {
    let game = interactive_game(4, 42);
    let current = game.current_player();
    let spectator = PlayerId(if current.0 == 0 { 1 } else { 0 });
    let mine = game.get_player_view(current);
    let theirs = game.get_player_view(spectator);
    assert!(
        mine.drawn_card.is_some(),
        "current player should see their drawn card in their own PlayerView"
    );
    assert!(
        theirs.drawn_card.is_none(),
        "spectator should not see the current player's drawn card"
    );
}

#[test]
fn full_state_reveals_drawn_card() {
    let game = interactive_game(4, 42);
    let state = game.get_full_state();
    assert!(
        state.drawn_card.is_some(),
        "InteractiveGameState::drawn_card should reveal the faceup drawn card"
    );
}

#[test]
fn player_view_hides_other_players_hands() {
    // Smoke test the `hands: Vec<Option<Vec<Card>>>` redaction even for
    // standard rules (hands are empty; we're verifying the None vs Some
    // positioning).
    let game = interactive_game(4, 42);
    let current = game.current_player();
    let view = game.get_player_view(current);
    assert_eq!(view.hands.len(), 4);
    for (p, h) in view.hands.iter().enumerate() {
        if PlayerId(p as u8) == current {
            assert!(h.is_some(), "viewer's own hand slot should be Some");
        } else {
            assert!(h.is_none(), "other player's hand slot should be None");
        }
    }
}
