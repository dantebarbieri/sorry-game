//! Per-strategy scenario tests. Each builds a hand-constructed position,
//! drives `choose_move` directly, and asserts the returned move reflects
//! the strategy's documented priority.
//!
//! The Loser-vs-Random smoke test at the bottom is a statistical check —
//! 500 seeded 2-player games — to verify Loser actually loses to Random
//! (its stated goal).

use rand::SeedableRng;
use rand::rngs::StdRng;
use sorry_core::{
    Card, Game, GreedyStrategy, LoserStrategy, Move, NotSorryStrategy, PawnId, PlayerId,
    RandomStrategy, ReverseStrategy, Rules, SidekickStrategy, SpaceId, StandardRules, Strategy,
    StrategyView, SurvivorStrategy, TeleporterStrategy, legal_moves,
};

fn fresh_view(rules: &StandardRules, me: PlayerId, num_players: usize) -> StrategyView {
    let start_areas: Vec<SpaceId> = (0..num_players as u8)
        .map(|p| rules.start_area(PlayerId(p)))
        .collect();
    let pawns_per = rules.pawns_per_player();
    let pawn_positions: Vec<Vec<SpaceId>> = (0..num_players)
        .map(|p| vec![start_areas[p]; pawns_per])
        .collect();
    StrategyView {
        my_player: me,
        num_players,
        hand: Vec::new(),
        drawn_card: None,
        pawn_positions,
        discard: Vec::new(),
        deck_remaining: 45,
        current_player_turn: me,
        last_bump_victim: None,
    }
}

fn fresh_board_state(
    rules: &StandardRules,
    num_players: usize,
) -> sorry_core::BoardState {
    let start_areas: Vec<SpaceId> = (0..num_players as u8)
        .map(|p| rules.start_area(PlayerId(p)))
        .collect();
    sorry_core::BoardState::fresh(num_players, rules.pawns_per_player(), &start_areas)
}

#[test]
fn greedy_picks_move_into_home() {
    // Use a Card::Five: doesn't start pawns (start_pawn_advance is None),
    // so the ONLY legal moves are the two Advances available. Pawn 0 at
    // safety_0 + 5 → home (distance drop 5, plus the into-safety tiebreak
    // fires for landing on home). Pawn 1 at track 20 + 5 → track 25
    // (same distance drop, no tiebreak bonus). Greedy must pick pawn 0.
    let rules = StandardRules::new();
    let p0 = PlayerId(0);
    let mut board = fresh_board_state(&rules, 2);
    board.set_position(p0, PawnId(0), rules.safety_space(p0, 0));
    board.set_position(p0, PawnId(1), rules.track_space(20));

    let legal = legal_moves(&rules, &board, p0, Card::Five);

    let mut view = fresh_view(&rules, p0, 2);
    view.pawn_positions[0][0] = rules.safety_space(p0, 0);
    view.pawn_positions[0][1] = rules.track_space(20);
    view.drawn_card = Some(Card::Five);

    let mut rng = StdRng::seed_from_u64(1);
    let choice = GreedyStrategy.choose_move(&view, &rules, Card::Five, &legal, &mut rng);
    match choice {
        Move::Advance { pawn, to, .. } => {
            assert_eq!(pawn, PawnId(0), "Greedy should pick pawn 0 into home");
            assert_eq!(to, rules.home(p0));
        }
        other => panic!("expected Advance → home, got {other:?}"),
    }
}

#[test]
fn not_sorry_starts_pawn_when_start_move_exists() {
    let rules = StandardRules::new();
    let p0 = PlayerId(0);
    let mut board = fresh_board_state(&rules, 2);
    // Pawn 1 on the track so Greedy would prefer advancing it.
    board.set_position(p0, PawnId(1), rules.track_space(20));
    // Pawn 0 stays in StartArea so a card 1 can start it.

    let legal = legal_moves(&rules, &board, p0, Card::One);
    let mut view = fresh_view(&rules, p0, 2);
    view.pawn_positions[0][1] = rules.track_space(20);
    view.drawn_card = Some(Card::One);

    let mut rng = StdRng::seed_from_u64(1);
    let choice = NotSorryStrategy.choose_move(&view, &rules, Card::One, &legal, &mut rng);
    assert!(
        matches!(choice, Move::StartPawn { .. }),
        "Not Sorry should prefer StartPawn; got {choice:?}"
    );
}

#[test]
fn survivor_bumps_leader_when_possible() {
    let rules = StandardRules::new();
    let p0 = PlayerId(0);
    let p1 = PlayerId(1);
    let mut board = fresh_board_state(&rules, 2);
    // Leader = p1. Put p1's pawn 0 close to their home (in safety slot 4)
    // so p1 clearly has the smallest mean distance-to-home. Put another
    // p1 pawn on the track at position 15 where p0 can bump.
    board.set_position(p1, PawnId(0), rules.safety_space(p1, 4));
    board.set_position(p1, PawnId(1), rules.track_space(15));
    // p0's pawn 0 at track 12 → Advance 3 → lands on track 15.
    board.set_position(p0, PawnId(0), rules.track_space(12));

    let legal = legal_moves(&rules, &board, p0, Card::Three);
    let mut view = fresh_view(&rules, p0, 2);
    view.pawn_positions[0][0] = rules.track_space(12);
    view.pawn_positions[1][0] = rules.safety_space(p1, 4);
    view.pawn_positions[1][1] = rules.track_space(15);
    view.drawn_card = Some(Card::Three);

    let mut rng = StdRng::seed_from_u64(1);
    let choice = SurvivorStrategy.choose_move(&view, &rules, Card::Three, &legal, &mut rng);
    match choice {
        Move::Advance { pawn, to, .. } => {
            assert_eq!(pawn, PawnId(0));
            assert_eq!(to, rules.track_space(15), "Survivor should bump leader at track 15");
        }
        other => panic!("expected Advance bumping leader; got {other:?}"),
    }
}

#[test]
fn teleporter_swaps_even_when_backward() {
    let rules = StandardRules::new();
    let p0 = PlayerId(0);
    let p1 = PlayerId(1);
    let mut board = fresh_board_state(&rules, 2);
    // p0's pawn 0 far along the track (track 57, near end of p0's lap).
    // p1's pawn 0 near track 0 — swap would move p0's pawn BACKWARD.
    board.set_position(p0, PawnId(0), rules.track_space(57));
    board.set_position(p1, PawnId(0), rules.track_space(10));

    let legal = legal_moves(&rules, &board, p0, Card::Eleven);
    let mut view = fresh_view(&rules, p0, 2);
    view.pawn_positions[0][0] = rules.track_space(57);
    view.pawn_positions[1][0] = rules.track_space(10);
    view.drawn_card = Some(Card::Eleven);

    let mut rng = StdRng::seed_from_u64(1);
    let choice = TeleporterStrategy.choose_move(&view, &rules, Card::Eleven, &legal, &mut rng);
    assert!(
        matches!(choice, Move::SwapEleven { .. }),
        "Teleporter should swap even when the swap moves its pawn backward; got {choice:?}"
    );
}

#[test]
fn sidekick_targets_last_bump_victim() {
    let rules = StandardRules::new();
    let p0 = PlayerId(0);
    let p1 = PlayerId(1);
    let p2 = PlayerId(2);
    let mut board = fresh_board_state(&rules, 3);
    // Leader by distance is p2 (deep in safety) but the recent bump
    // victim is p1 — Sidekick must attack p1 specifically.
    board.set_position(p2, PawnId(0), rules.safety_space(p2, 3));
    board.set_position(p1, PawnId(0), rules.track_space(15));
    board.set_position(p0, PawnId(0), rules.track_space(12));

    let legal = legal_moves(&rules, &board, p0, Card::Three);
    let mut view = fresh_view(&rules, p0, 3);
    view.pawn_positions[0][0] = rules.track_space(12);
    view.pawn_positions[1][0] = rules.track_space(15);
    view.pawn_positions[2][0] = rules.safety_space(p2, 3);
    view.last_bump_victim = Some(p1);
    view.drawn_card = Some(Card::Three);

    let mut rng = StdRng::seed_from_u64(1);
    let choice = SidekickStrategy.choose_move(&view, &rules, Card::Three, &legal, &mut rng);
    match choice {
        Move::Advance { to, .. } => {
            assert_eq!(to, rules.track_space(15), "Sidekick should target recent victim p1");
        }
        other => panic!("expected bump of victim; got {other:?}"),
    }
}

#[test]
fn reverse_holds_primary_near_start_exit_on_non_4_10_card() {
    let rules = StandardRules::new();
    let p0 = PlayerId(0);
    let mut board = fresh_board_state(&rules, 2);
    // Primary pawn 0 sitting exactly on start_exit (track 4 for p0).
    // Pawn 1 sits far along the track so Reverse has an alternative move.
    board.set_position(p0, PawnId(0), rules.start_exit(p0));
    board.set_position(p0, PawnId(1), rules.track_space(30));

    // Drawing a Three. Plenty of 4s and 10s still in the deck (discard
    // empty), so Reverse's hoard_remaining > 0 and it should avoid
    // advancing the primary.
    let legal = legal_moves(&rules, &board, p0, Card::Three);
    let mut view = fresh_view(&rules, p0, 2);
    view.pawn_positions[0][0] = rules.start_exit(p0);
    view.pawn_positions[0][1] = rules.track_space(30);
    view.drawn_card = Some(Card::Three);

    let mut rng = StdRng::seed_from_u64(1);
    let choice = ReverseStrategy.choose_move(&view, &rules, Card::Three, &legal, &mut rng);
    // Expectation: the chosen move moves pawn 1, not pawn 0 — Reverse
    // holds the primary.
    match choice {
        Move::Advance { pawn, .. } => {
            assert_eq!(pawn, PawnId(1), "Reverse should not advance primary on a 3");
        }
        other => panic!("expected Advance of pawn 1; got {other:?}"),
    }
}

// ─── Loser tests ─────────────────────────────────────────────────────

#[test]
fn loser_prefers_non_bumping_move_over_bumping_one() {
    // Bump-avoidance is Loser's PRIMARY priority. If any non-bumping
    // legal move exists, Loser must pick one — even a forward advance —
    // over a bumping alternative.
    let rules = StandardRules::new();
    let p0 = PlayerId(0);
    let p1 = PlayerId(1);
    let mut board = fresh_board_state(&rules, 2);
    // p0 pawn 0 at track 10; +3 → track 13 where an opp pawn sits (BUMP).
    // p0 pawn 1 at track 40; +3 → track 43 (no pawn there).
    board.set_position(p0, PawnId(0), rules.track_space(10));
    board.set_position(p0, PawnId(1), rules.track_space(40));
    board.set_position(p1, PawnId(0), rules.track_space(13));

    let legal = legal_moves(&rules, &board, p0, Card::Three);
    let mut view = fresh_view(&rules, p0, 2);
    view.pawn_positions[0][0] = rules.track_space(10);
    view.pawn_positions[0][1] = rules.track_space(40);
    view.pawn_positions[1][0] = rules.track_space(13);
    view.drawn_card = Some(Card::Three);

    let mut rng = StdRng::seed_from_u64(1);
    let choice = LoserStrategy.choose_move(&view, &rules, Card::Three, &legal, &mut rng);
    match choice {
        Move::Advance { pawn, to, .. } => {
            assert_eq!(pawn, PawnId(1), "Loser must not bump when pawn 1 has a safe alternative");
            assert_eq!(to, rules.track_space(43));
        }
        other => panic!("expected non-bumping Advance of pawn 1; got {other:?}"),
    }
}

#[test]
fn loser_prefers_backward_one_over_forward_ten() {
    // With a 10, both forward-10 and back-1 are legal for a pawn in open
    // track. Loser should pick the retreat since it increases own
    // distance-to-home.
    let rules = StandardRules::new();
    let p0 = PlayerId(0);
    let mut board = fresh_board_state(&rules, 2);
    // Pawn 0 on track 30 — plenty of room both forward and backward.
    board.set_position(p0, PawnId(0), rules.track_space(30));

    let legal = legal_moves(&rules, &board, p0, Card::Ten);
    let mut view = fresh_view(&rules, p0, 2);
    view.pawn_positions[0][0] = rules.track_space(30);
    view.drawn_card = Some(Card::Ten);

    let mut rng = StdRng::seed_from_u64(1);
    let choice = LoserStrategy.choose_move(&view, &rules, Card::Ten, &legal, &mut rng);
    assert!(
        matches!(choice, Move::Retreat { .. }),
        "Loser should retreat on a 10 when no bump is involved; got {choice:?}"
    );
}

#[test]
fn loser_takes_eleven_swap_that_sabotages_self() {
    // An 11 that swaps our advanced pawn with a far-back opponent pawn —
    // the swap moves our pawn backward AND moves the opponent's pawn
    // closer to their home. Loser's self-harm scoring should pick the
    // swap over a plain Advance-11.
    let rules = StandardRules::new();
    let p0 = PlayerId(0);
    let p1 = PlayerId(1);
    let mut board = fresh_board_state(&rules, 2);
    // p0 on track 40 — closer-to-home for p0.
    // p1 on track 20 — near p1's home side.
    board.set_position(p0, PawnId(0), rules.track_space(40));
    board.set_position(p1, PawnId(0), rules.track_space(20));

    let legal = legal_moves(&rules, &board, p0, Card::Eleven);
    let mut view = fresh_view(&rules, p0, 2);
    view.pawn_positions[0][0] = rules.track_space(40);
    view.pawn_positions[1][0] = rules.track_space(20);
    view.drawn_card = Some(Card::Eleven);

    let mut rng = StdRng::seed_from_u64(1);
    let choice = LoserStrategy.choose_move(&view, &rules, Card::Eleven, &legal, &mut rng);
    assert!(
        matches!(choice, Move::SwapEleven { .. }),
        "Loser should choose the self-sabotaging swap; got {choice:?}"
    );
}

// ─── Loser-vs-Random smoke test ───────────────────────────────────────

#[test]
fn loser_loses_most_games_against_random() {
    // Run 400 seeded 2-player games, alternating seats so Loser plays
    // each position. Assert Random wins at least 60% — Loser is
    // designed to lose to Random; observed rate is ~65% with this
    // implementation. The 60% floor leaves ~5% margin for run-to-run
    // variance at this sample size (2σ ≈ 4.7% around a 65% true winrate).
    let num_games = 400;
    let mut random_wins = 0u32;
    let mut loser_wins = 0u32;
    let mut truncated = 0u32;

    for seed in 0..num_games {
        // Seat flip: even seeds put Random at seat 0, odd seeds at seat 1.
        let random_seat = (seed % 2) as u8;
        let strategies: Vec<Box<dyn Strategy>> = if random_seat == 0 {
            vec![Box::new(RandomStrategy), Box::new(LoserStrategy)]
        } else {
            vec![Box::new(LoserStrategy), Box::new(RandomStrategy)]
        };
        let rules: Box<dyn Rules> = Box::new(StandardRules::new());
        let mut game = Game::new(rules, strategies, seed).expect("new game");
        game.set_max_turns(4_000);
        let history = game.play().expect("play");
        if history.truncated {
            truncated += 1;
            continue;
        }
        assert_eq!(history.winners.len(), 1, "Standard rules: exactly one winner");
        let winner = history.winners[0];
        if winner.0 == random_seat {
            random_wins += 1;
        } else {
            loser_wins += 1;
        }
    }
    let decided = random_wins + loser_wins;
    assert!(decided > 0, "all games truncated — unexpected");
    let rate = random_wins as f32 / decided as f32;
    assert!(
        rate >= 0.60,
        "Random won {random_wins}/{decided} ({:.1}%) vs Loser; truncated={truncated}. \
         Expected >= 60%.",
        rate * 100.0
    );
}
