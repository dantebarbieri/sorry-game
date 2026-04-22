//! Hand-constructed board scenarios to exercise specific rules.

use sorry_core::{
    BoardState, BumpEvent, Card, Move, PawnId, PlayerId, Rules, SlideEvent, Space, SpaceId,
    StandardRules, apply_move, legal_moves,
};

fn fresh_board(rules: &StandardRules, num_players: usize) -> BoardState {
    let start_areas: Vec<_> = (0..num_players as u8)
        .map(|p| rules.start_area(PlayerId(p)))
        .collect();
    BoardState::fresh(num_players, rules.pawns_per_player(), &start_areas)
}

#[test]
fn bump_sends_opponent_to_start() {
    let rules = StandardRules::new();
    let mut board = fresh_board(&rules, 4);
    let p0 = PlayerId(0);
    let p1 = PlayerId(1);
    // Park player 1's pawn 0 at track 15 (their start-exit). Not a slide head.
    board.set_position(p1, PawnId(0), rules.track_space(15));
    // Player 0's pawn 0 at track 12, playing a 3 → lands on track 15.
    board.set_position(p0, PawnId(0), rules.track_space(12));

    let legal = legal_moves(&rules, &board, p0, Card::Three);
    let mv = legal
        .iter()
        .find(|m| matches!(m, Move::Advance { to, .. } if *to == rules.track_space(15)))
        .cloned()
        .expect("advance to track 15 should be legal");
    let (bumps, slides) = apply_move(&rules, &mut board, p0, &mv);

    assert_eq!(board.position(p0, PawnId(0)), rules.track_space(15));
    assert_eq!(board.position(p1, PawnId(0)), rules.start_area(p1));
    assert_eq!(bumps.len(), 1);
    assert!(slides.is_empty());
    assert_eq!(
        bumps[0],
        BumpEvent {
            player: p1,
            pawn: PawnId(0),
            from: rules.track_space(15),
            to: rules.start_area(p1),
        }
    );
}

#[test]
fn cannot_land_on_own_pawn_move_filtered_from_legal_set() {
    let rules = StandardRules::new();
    let mut board = fresh_board(&rules, 4);
    let p0 = PlayerId(0);
    // Player 0 has two pawns adjacent on the track. A forward move from one
    // that would land on the other must be filtered.
    board.set_position(p0, PawnId(0), rules.track_space(20));
    board.set_position(p0, PawnId(1), rules.track_space(23));

    let legal = legal_moves(&rules, &board, p0, Card::Three);
    // Pawn 0 moving forward 3 would land on track 23 (own pawn 1) — disallowed.
    let illegal = legal.iter().any(|m| {
        matches!(
            m,
            Move::Advance { pawn, to, .. } if *pawn == PawnId(0) && *to == rules.track_space(23)
        )
    });
    assert!(!illegal, "landing on own pawn must be filtered out");

    // Pawn 1 moving forward 3 to track 26 should remain legal (empty space).
    let pawn1_move = legal.iter().any(|m| {
        matches!(
            m,
            Move::Advance { pawn, to, .. } if *pawn == PawnId(1) && *to == rules.track_space(26)
        )
    });
    assert!(pawn1_move);
}

#[test]
fn no_legal_move_forfeits_turn() {
    let rules = StandardRules::new();
    let board = fresh_board(&rules, 4);
    let p0 = PlayerId(0);
    // All player 0 pawns are in StartArea. A Three has no effect (can't
    // start a pawn with 3, and no pawns are on the track).
    let legal = legal_moves(&rules, &board, p0, Card::Three);
    assert_eq!(legal, vec![Move::Pass]);
}

#[test]
fn own_color_slide_does_not_trigger() {
    let rules = StandardRules::new();
    let mut board = fresh_board(&rules, 4);
    let p0 = PlayerId(0);
    // Player 0 at track 0 plays a 1 → lands on track 1 (side 0 short slide
    // head, player 0's color). Should not trigger slide.
    board.set_position(p0, PawnId(0), rules.track_space(0));

    let legal = legal_moves(&rules, &board, p0, Card::One);
    let mv = legal
        .iter()
        .find(|m| matches!(m, Move::Advance { to, .. } if *to == rules.track_space(1)))
        .cloned()
        .expect("advance to track 1 should be legal");
    let (bumps, slides) = apply_move(&rules, &mut board, p0, &mv);

    assert!(slides.is_empty(), "own-color slide must not trigger");
    assert!(bumps.is_empty());
    assert_eq!(board.position(p0, PawnId(0)), rules.track_space(1));
}

#[test]
fn different_color_slide_triggers_and_bumps_all_path_pawns_including_own() {
    let rules = StandardRules::new();
    let mut board = fresh_board(&rules, 4);
    let p1 = PlayerId(1);
    let p2 = PlayerId(2);
    let p3 = PlayerId(3);
    // Player 1's pawn lands on side-0 short slide head (track 1) via a Two.
    board.set_position(p1, PawnId(0), rules.track_space(59));
    // Park an opponent (player 2) pawn on the slide path at track 3.
    board.set_position(p2, PawnId(0), rules.track_space(3));
    // Park another opponent (player 3) pawn on the slide end at track 4.
    board.set_position(p3, PawnId(0), rules.track_space(4));

    let legal = legal_moves(&rules, &board, p1, Card::Two);
    let mv = legal
        .iter()
        .find(|m| {
            matches!(m, Move::Advance { pawn, to, .. }
            if *pawn == PawnId(0) && *to == rules.track_space(1))
        })
        .cloned()
        .expect("advance pawn 0 to track 1 should be legal");
    let (bumps, slides) = apply_move(&rules, &mut board, p1, &mv);

    // Slide moves player 1's pawn from track 1 → track 4.
    assert_eq!(board.position(p1, PawnId(0)), rules.track_space(4));
    assert_eq!(slides.len(), 1);
    let slide: &SlideEvent = &slides[0];
    assert_eq!(slide.from, rules.track_space(1));
    assert_eq!(slide.to, rules.track_space(4));

    // Both opponent pawns on the slide path are bumped to their starts.
    assert_eq!(board.position(p2, PawnId(0)), rules.start_area(p2));
    assert_eq!(board.position(p3, PawnId(0)), rules.start_area(p3));
    assert_eq!(bumps.len(), 2);
}

#[test]
fn safety_zone_requires_exact_count() {
    let rules = StandardRules::new();
    let mut board = fresh_board(&rules, 4);
    let p0 = PlayerId(0);
    // Player 0's pawn at safety slot 3. A 5 would overshoot Home (safety 3
    // → 4 → Home → illegal fifth step).
    board.set_position(p0, PawnId(0), rules.safety_space(p0, 3));

    let legal = legal_moves(&rules, &board, p0, Card::Five);
    // Pawn 0 should have no legal Advance from safety slot 3 with 5.
    let any_advance_from_pawn_0 = legal.iter().any(|m| {
        matches!(
            m,
            Move::Advance { pawn, .. } if *pawn == PawnId(0)
        )
    });
    assert!(
        !any_advance_from_pawn_0,
        "overshooting home should be illegal"
    );

    // But a 2 from safety 3 → safety 4 → Home is legal.
    let legal_two = legal_moves(&rules, &board, p0, Card::Two);
    let can_reach_home = legal_two.iter().any(|m| {
        matches!(
            m,
            Move::Advance { pawn, to, .. }
                if *pawn == PawnId(0) && *to == rules.home(p0)
        )
    });
    assert!(
        can_reach_home,
        "2 should land exactly on home from safety 3"
    );
}

#[test]
fn sorry_card_swaps_start_for_opponent_track_pawn() {
    let rules = StandardRules::new();
    let mut board = fresh_board(&rules, 4);
    let p0 = PlayerId(0);
    let p1 = PlayerId(1);
    // Player 0 has all pawns in start. Player 1 has a pawn at track 30.
    board.set_position(p1, PawnId(0), rules.track_space(30));

    let legal = legal_moves(&rules, &board, p0, Card::Sorry);
    let mv = legal
        .iter()
        .find(|m| matches!(m, Move::Sorry { their_player, .. } if *their_player == p1))
        .cloned()
        .expect("Sorry targeting player 1 should be legal");
    let (bumps, _slides) = apply_move(&rules, &mut board, p0, &mv);

    // Player 1's pawn sent to start; player 0's pawn moved to track 30.
    assert_eq!(board.position(p1, PawnId(0)), rules.start_area(p1));
    assert!(
        board
            .pawns_of(p0)
            .iter()
            .any(|s| *s == rules.track_space(30))
    );
    assert!(!bumps.is_empty());
}

#[test]
fn sorry_and_swap_eleven_cannot_target_own_pawn() {
    let rules = StandardRules::new();
    let mut board = fresh_board(&rules, 4);
    let p0 = PlayerId(0);
    // Player 0 has one pawn in start and one pawn on the track. Sorry
    // should have NO legal move targeting the own track pawn.
    board.set_position(p0, PawnId(0), rules.track_space(25));
    // Pawn 1 stays in start_area.

    let legal_sorry = legal_moves(&rules, &board, p0, Card::Sorry);
    let any_targets_self = legal_sorry.iter().any(|m| {
        matches!(
            m,
            Move::Sorry { their_player, .. } if *their_player == p0
        )
    });
    assert!(!any_targets_self);

    // With an opponent off-track, Sorry has no valid target and must Pass.
    assert_eq!(legal_sorry, vec![Move::Pass]);

    // Same for SwapEleven: two of own pawns on track, no opponents on
    // track.
    board.set_position(p0, PawnId(2), rules.track_space(35));
    let legal_eleven = legal_moves(&rules, &board, p0, Card::Eleven);
    let any_swap_self = legal_eleven.iter().any(|m| {
        matches!(
            m,
            Move::SwapEleven { their_player, .. } if *their_player == p0
        )
    });
    assert!(!any_swap_self);
}

#[test]
fn seven_split_emits_distinct_pawn_partitions() {
    let rules = StandardRules::new();
    let mut board = fresh_board(&rules, 4);
    let p0 = PlayerId(0);
    // Two player-0 pawns on the track, plenty of room to split.
    board.set_position(p0, PawnId(0), rules.track_space(20));
    board.set_position(p0, PawnId(1), rules.track_space(40));

    let legal = legal_moves(&rules, &board, p0, Card::Seven);
    let mut split_count = 0;
    let mut whole_count = 0;
    for m in &legal {
        match m {
            Move::SplitSeven { first, second } => {
                assert_ne!(first.pawn, second.pawn);
                assert_eq!(first.steps + second.steps, 7);
                assert!(first.steps >= 1 && second.steps >= 1);
                split_count += 1;
            }
            Move::Advance { card_value: 7, .. } => {
                whole_count += 1;
            }
            _ => {}
        }
    }
    assert!(split_count > 0, "should emit at least one split");
    assert!(whole_count > 0, "should emit at least one whole-7 advance");
}

#[test]
fn one_card_starts_pawn_on_start_exit() {
    let rules = StandardRules::new();
    let board = fresh_board(&rules, 4);
    let p0 = PlayerId(0);
    let legal = legal_moves(&rules, &board, p0, Card::One);
    let start_exit = rules.start_exit(p0);
    let found = legal
        .iter()
        .find(|m| matches!(m, Move::StartPawn { to, .. } if *to == start_exit));
    assert!(
        found.is_some(),
        "expected StartPawn → {start_exit:?} for card 1; got {legal:?}"
    );
}

#[test]
fn two_card_starts_pawn_one_past_start_exit() {
    let rules = StandardRules::new();
    let board = fresh_board(&rules, 4);
    let p0 = PlayerId(0);
    let legal = legal_moves(&rules, &board, p0, Card::Two);
    let start_exit = rules.start_exit(p0);
    let past = rules
        .forward_neighbor(start_exit, p0)
        .expect("start_exit has forward neighbor");
    let found = legal
        .iter()
        .find(|m| matches!(m, Move::StartPawn { to, .. } if *to == past));
    assert!(
        found.is_some(),
        "expected StartPawn → {past:?} (one past start_exit) for card 2; got {legal:?}"
    );
    // And specifically NOT one to start_exit itself — 2 doesn't stop there.
    assert!(
        !legal
            .iter()
            .any(|m| matches!(m, Move::StartPawn { to, .. } if *to == start_exit)),
        "card 2 must not produce StartPawn → start_exit; got {legal:?}"
    );
}

#[test]
fn two_card_blocked_if_own_pawn_on_start_exit() {
    let rules = StandardRules::new();
    let mut board = fresh_board(&rules, 4);
    let p0 = PlayerId(0);
    let start_exit = rules.start_exit(p0);
    // Pawn 0 already parked on start_exit; pawn 1 still in Start.
    board.set_position(p0, PawnId(0), start_exit);
    let legal = legal_moves(&rules, &board, p0, Card::Two);
    assert!(
        !legal.iter().any(|m| matches!(m, Move::StartPawn { .. })),
        "card 2 must be blocked when own pawn sits on start_exit; got {legal:?}"
    );
}

#[test]
fn two_card_blocked_if_own_pawn_one_past_start_exit() {
    let rules = StandardRules::new();
    let mut board = fresh_board(&rules, 4);
    let p0 = PlayerId(0);
    let start_exit = rules.start_exit(p0);
    let past = rules.forward_neighbor(start_exit, p0).unwrap();
    // start_exit clear, but the 2's landing square is occupied by our own pawn.
    board.set_position(p0, PawnId(0), past);
    let legal = legal_moves(&rules, &board, p0, Card::Two);
    assert!(
        !legal.iter().any(|m| matches!(m, Move::StartPawn { .. })),
        "card 2 must be blocked when own pawn sits one past start_exit; got {legal:?}"
    );
}

#[test]
fn one_card_not_blocked_by_own_pawn_one_past_start_exit() {
    // The 1-card only cares about start_exit itself. Having your own pawn
    // one space further is irrelevant.
    let rules = StandardRules::new();
    let mut board = fresh_board(&rules, 4);
    let p0 = PlayerId(0);
    let start_exit = rules.start_exit(p0);
    let past = rules.forward_neighbor(start_exit, p0).unwrap();
    board.set_position(p0, PawnId(0), past);
    let legal = legal_moves(&rules, &board, p0, Card::One);
    assert!(
        legal
            .iter()
            .any(|m| matches!(m, Move::StartPawn { to, .. } if *to == start_exit)),
        "card 1 should still start a pawn onto start_exit; got {legal:?}"
    );
}

#[test]
fn space_enum_used_through_trait_classify() {
    // Sanity: the Rules trait's classify() returns Space enum correctly.
    let rules = StandardRules::new();
    let s = rules.classify(SpaceId(0)).expect("classify");
    assert_eq!(s, Space::Track(0));
    assert_eq!(
        rules.classify(rules.start_area(PlayerId(2))),
        Some(Space::StartArea(PlayerId(2)))
    );
    assert_eq!(
        rules.classify(rules.home(PlayerId(3))),
        Some(Space::Home(PlayerId(3)))
    );
}
