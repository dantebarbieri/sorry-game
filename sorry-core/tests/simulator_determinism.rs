//! Guards the native ↔ WASM determinism contract by pinning a reference
//! history that `sorry-wasm`'s `simulate_one_with_history` must reproduce
//! byte-for-byte for the same seed + config. If this test changes, the
//! WASM layer's output must change in lockstep.

use sorry_core::{Card, Game, GameHistory, RandomStrategy, Rules, StandardRules, Strategy};

fn play(seed: u64, num_players: usize) -> GameHistory {
    let rules: Box<dyn Rules> = Box::new(StandardRules::new());
    let strategies: Vec<Box<dyn Strategy>> = (0..num_players)
        .map(|_| Box::new(RandomStrategy) as Box<dyn Strategy>)
        .collect();
    Game::new(rules, strategies, seed)
        .expect("construct game")
        .play()
        .expect("run game")
}

#[test]
fn reference_history_for_seed_2026_3p_is_stable() {
    let a = play(2026, 3);
    let b = play(2026, 3);
    assert_eq!(a, b, "native replay must be deterministic");
    assert_eq!(a.seed, 2026);
    assert_eq!(a.num_players, 3);
    assert_eq!(a.strategy_names, vec!["Random", "Random", "Random"]);
    assert_eq!(a.rules_name, "Standard");

    // Pins the first five cards of the seeded deck. The WASM smoke test
    // reads the same five from `simulate_one_with_history` — if these two
    // diverge, the ChaCha12 determinism promise (game.rs:1-4) is broken.
    assert_eq!(
        &a.initial_deck_order[..5],
        &[Card::Three, Card::Ten, Card::One, Card::Ten, Card::Ten]
    );
}
