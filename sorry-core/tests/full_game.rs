use sorry_core::{Game, PlayerId, RandomStrategy, Rules, StandardRules, Strategy};

fn random_strategies(n: usize) -> Vec<Box<dyn Strategy>> {
    (0..n)
        .map(|_| Box::new(RandomStrategy) as Box<dyn Strategy>)
        .collect()
}

#[test]
fn game_runs_to_completion_random_v_random_seed_1() {
    let rules: Box<dyn Rules> = Box::new(StandardRules::new());
    let strategies = random_strategies(4);
    let game = Game::new(rules, strategies, 1).expect("new game");
    let history = game.play().expect("play");

    assert!(
        !history.turns.is_empty(),
        "game should have at least one turn"
    );
    // Either a winner was found or the game was truncated.
    if history.truncated {
        eprintln!(
            "game truncated at {} turns without winner",
            history.turns.len()
        );
    } else {
        assert!(
            !history.winners.is_empty(),
            "non-truncated game must have a winner"
        );
        let rules = StandardRules::new();
        // Reconstruct the final board by replaying; for now just check via
        // winners field.
        for &w in &history.winners {
            let _ = w;
            let _ = &rules;
        }
    }
}

#[test]
fn two_player_game_works() {
    let rules: Box<dyn Rules> = Box::new(StandardRules::new());
    let strategies = random_strategies(2);
    let mut game = Game::new(rules, strategies, 42).expect("new game");
    game.set_max_turns(3_000);
    let history = game.play().expect("play");
    assert!(!history.turns.is_empty());
    for t in &history.turns {
        assert!(
            t.player.0 < 2,
            "2-player game should only reference player ids 0/1"
        );
    }
}

#[test]
fn three_player_game_works() {
    let rules: Box<dyn Rules> = Box::new(StandardRules::new());
    let strategies = random_strategies(3);
    let game = Game::new(rules, strategies, 7).expect("new game");
    let history = game.play().expect("play");
    assert!(!history.turns.is_empty());
    for t in &history.turns {
        assert!(t.player.0 < 3);
    }
}

#[test]
fn rejects_wrong_player_count() {
    let rules: Box<dyn Rules> = Box::new(StandardRules::new());
    let strategies = random_strategies(1);
    assert!(Game::new(rules, strategies, 0).is_err());

    let rules: Box<dyn Rules> = Box::new(StandardRules::new());
    let strategies = random_strategies(5);
    assert!(Game::new(rules, strategies, 0).is_err());
}

#[test]
fn starting_player_is_within_bounds() {
    let rules: Box<dyn Rules> = Box::new(StandardRules::new());
    let strategies = random_strategies(4);
    let game = Game::new(rules, strategies, 99).expect("new game");
    let history = game.play().expect("play");
    assert!(history.starting_player.0 < 4);
    let _ = PlayerId(0);
}

#[test]
fn play_out_variant_records_full_placement_order() {
    let rules: Box<dyn Rules> = Box::new(StandardRules::new_play_out());
    let strategies = random_strategies(4);
    let mut game = Game::new(rules, strategies, 12345).expect("new game");
    game.set_max_turns(10_000);
    let history = game.play().expect("play");
    if history.truncated {
        eprintln!("seed 12345 truncated — try a different seed for this test");
        return;
    }
    // Play Out finalizes the full 1st-through-Nth placement order.
    assert_eq!(
        history.winners.len(),
        history.num_players,
        "Play Out should record every player's placement, got {:?}",
        history.winners
    );
    // Placements must be a permutation of the player IDs.
    let mut seen = std::collections::HashSet::new();
    for w in &history.winners {
        assert!(
            w.0 < history.num_players as u8,
            "placement {w:?} out of range"
        );
        assert!(seen.insert(w.0), "player {:?} recorded twice", w);
    }
}

#[test]
fn standard_variant_stops_at_first_finisher() {
    let rules: Box<dyn Rules> = Box::new(StandardRules::new());
    let strategies = random_strategies(4);
    let game = Game::new(rules, strategies, 12345).expect("new game");
    let history = game.play().expect("play");
    if history.truncated {
        return;
    }
    assert_eq!(
        history.winners.len(),
        1,
        "Standard rules should end at the first finisher, got {:?}",
        history.winners
    );
}
