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
fn advance_to_home_is_legal_even_when_home_already_holds_own_pawn() {
    // Regression: `is_own_pawn_at` was blocking any move to Home once a
    // pawn had already parked there, which breaks late-game play. Home
    // is a stacking space (that's how you win).
    let rules = StandardRules::new();
    let mut board = fresh_board(&rules, 4);
    let p0 = PlayerId(0);
    let home = rules.home(p0);
    // Park pawn 1 at Home already.
    board.set_position(p0, PawnId(1), home);
    // Pawn 0 sits one step away, in the last safety slot.
    board.set_position(p0, PawnId(0), rules.safety_space(p0, 4));
    let legal = legal_moves(&rules, &board, p0, Card::One);
    let advance = legal.iter().find(|m| matches!(
        m,
        Move::Advance { pawn, to, .. } if *pawn == PawnId(0) && *to == home
    ));
    assert!(
        advance.is_some(),
        "expected Advance pawn 0 → Home even with pawn 1 already at Home; got {legal:?}"
    );
}

#[test]
fn split_seven_allows_both_legs_to_home() {
    // Regression for the `to_a == to_b` early-rejection: Home is a
    // stacking space, so a 7 split whose legs each land at Home (e.g.
    // 5+2 from the safety zone) is legal.
    let rules = StandardRules::new();
    let mut board = fresh_board(&rules, 4);
    let p0 = PlayerId(0);
    let home = rules.home(p0);
    // Pawn 0 in the first safety slot — 5 steps from Home.
    board.set_position(p0, PawnId(0), rules.safety_space(p0, 0));
    // Pawn 1 in the 4th safety slot — 2 steps from Home.
    board.set_position(p0, PawnId(1), rules.safety_space(p0, 3));
    // The other two pawns stay in Start (default), where they can't
    // contribute alternative splits.

    let legal = legal_moves(&rules, &board, p0, Card::Seven);
    let both_home = legal.iter().find(|m| matches!(
        m,
        Move::SplitSeven { first, second }
            if first.pawn == PawnId(0) && first.steps == 5 && first.to == home
               && second.pawn == PawnId(1) && second.steps == 2 && second.to == home
    ));
    assert!(
        both_home.is_some(),
        "expected Split 7 as pawn 0 (5 steps) + pawn 1 (2 steps) both → Home; got {legal:?}"
    );
}

#[test]
fn split_seven_leg_to_home_legal_when_other_pawn_already_home() {
    // If one pawn is already at Home, a split that sends another pawn
    // there should still be legal — Home stacks.
    let rules = StandardRules::new();
    let mut board = fresh_board(&rules, 4);
    let p0 = PlayerId(0);
    let home = rules.home(p0);
    board.set_position(p0, PawnId(2), home); // already home
    board.set_position(p0, PawnId(0), rules.safety_space(p0, 4)); // 1 from home
    board.set_position(p0, PawnId(1), rules.track_space(25));
    // Leave pawn 3 in Start (default).

    let legal = legal_moves(&rules, &board, p0, Card::Seven);
    // Some Split-7 exists whose first leg is pawn 0 going 1 step to Home
    // (paired with pawn 1's 6-step advance).
    let has_home_split = legal.iter().any(|m| matches!(
        m,
        Move::SplitSeven { first, second }
            if first.pawn == PawnId(0) && first.to == home && second.pawn == PawnId(1)
    ));
    assert!(
        has_home_split,
        "expected a Split 7 sending pawn 0 home while pawn 1 advances; got {legal:?}"
    );
}

#[test]
fn eleven_allows_pass_when_advance_blocked_but_swap_available() {
    // Per the Hasbro rule text, a player who cannot move 11 forward is
    // *not* forced to swap — ending the turn is a legal alternative.
    // Construct: pawn 0 on the outer track whose +11 destination is
    // Safety(3) (for P0), which is blocked by our own pawn 1. Pawns 2
    // and 3 stay in Start so they can't Advance. Opponent pawn on the
    // outer track to make swap available.
    let rules = StandardRules::new();
    let mut board = fresh_board(&rules, 4);
    let p0 = PlayerId(0);
    let p1 = PlayerId(1);
    // Walking 11 from track 55 lands in Safety(3) for P0:
    //   55 → 56 → 57 → 58 → 59 → 0 → 1 → 2 → Safety(0) → Safety(1) →
    //   Safety(2) → Safety(3).
    board.set_position(p0, PawnId(0), rules.track_space(55));
    board.set_position(p0, PawnId(1), rules.safety_space(p0, 3));
    // Opponent pawn on the outer track — enables swap.
    board.set_position(p1, PawnId(0), rules.track_space(30));

    let legal = legal_moves(&rules, &board, p0, Card::Eleven);
    assert!(
        !legal.iter().any(|m| matches!(m, Move::Advance { .. })),
        "Advance-11 shouldn't be legal (own pawn blocks the landing); got {legal:?}"
    );
    assert!(
        legal.iter().any(|m| matches!(m, Move::SwapEleven { .. })),
        "expected a SwapEleven option; got {legal:?}"
    );
    assert!(
        legal.iter().any(|m| matches!(m, Move::Pass)),
        "expected Pass as an optional forfeit when Advance-11 is blocked; got {legal:?}"
    );
}

#[test]
fn eleven_does_not_offer_pass_when_advance_is_legal() {
    // The Pass escape is only there when you *can't* move 11; otherwise
    // the normal "use your card if you can" rule applies.
    let rules = StandardRules::new();
    let mut board = fresh_board(&rules, 4);
    let p0 = PlayerId(0);
    board.set_position(p0, PawnId(0), rules.track_space(5));
    // No own pawn at track 16 this time — Advance-11 is legal.
    let legal = legal_moves(&rules, &board, p0, Card::Eleven);
    assert!(
        legal.iter().any(|m| matches!(m, Move::Advance { card_value: 11, .. })),
        "expected Advance-11 to be legal; got {legal:?}"
    );
    assert!(
        !legal.iter().any(|m| matches!(m, Move::Pass)),
        "Pass must not be offered when Advance-11 is available; got {legal:?}"
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
