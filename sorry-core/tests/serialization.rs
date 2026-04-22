use sorry_core::{Game, GameHistory, RandomStrategy, Rules, StandardRules, Strategy};

fn run_game() -> GameHistory {
    let rules: Box<dyn Rules> = Box::new(StandardRules::new());
    let strategies: Vec<Box<dyn Strategy>> = (0..4)
        .map(|_| Box::new(RandomStrategy) as Box<dyn Strategy>)
        .collect();
    let game = Game::new(rules, strategies, 777).expect("new game");
    game.play().expect("play")
}

#[test]
fn history_json_roundtrip() {
    let history = run_game();
    let json = serde_json::to_string(&history).expect("serialize");
    let decoded: GameHistory = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(history, decoded);
}

#[test]
fn history_json_is_pretty_printable() {
    let history = run_game();
    let pretty = serde_json::to_string_pretty(&history).expect("pretty");
    assert!(pretty.contains("\"seed\""));
    assert!(pretty.contains("\"turns\""));
    assert!(pretty.contains("\"rules_name\""));
}
