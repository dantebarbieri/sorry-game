use sorry_core::{Game, RandomStrategy, Rules, StandardRules, Strategy};

fn run_seed(seed: u64, num_players: usize) -> sorry_core::GameHistory {
    let rules: Box<dyn Rules> = Box::new(StandardRules::new());
    let strategies: Vec<Box<dyn Strategy>> = (0..num_players)
        .map(|_| Box::new(RandomStrategy) as Box<dyn Strategy>)
        .collect();
    let game = Game::new(rules, strategies, seed).expect("new game");
    game.play().expect("play")
}

#[test]
fn same_seed_produces_identical_history() {
    let a = run_seed(1234, 4);
    let b = run_seed(1234, 4);
    assert_eq!(a, b, "identical seeds must produce identical histories");
}

#[test]
fn different_seeds_diverge() {
    let a = run_seed(1, 4);
    let b = run_seed(2, 4);
    // Not a strict guarantee but overwhelmingly likely given 45-card shuffle
    // space; treat equality as an unexpected collision.
    assert_ne!(a, b, "different seeds should diverge");
}

#[test]
fn initial_deck_order_is_deterministic() {
    let a = run_seed(55, 3);
    let b = run_seed(55, 3);
    assert_eq!(a.initial_deck_order, b.initial_deck_order);
    assert_eq!(a.initial_deck_order.len(), 45);
}
