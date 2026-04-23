//! Rules and Strategy factories. Mirrors `sorry-wasm/src/lib.rs:27-56` so
//! the set of rulesets/strategies exposed to networked clients stays in sync
//! with the WASM bridge. Add a new variant by editing both.

use sorry_core::{
    GreedyStrategy, LoserStrategy, NotSorryStrategy, RandomStrategy, ReverseStrategy, Rules,
    SidekickStrategy, StandardRules, Strategy, SurvivorStrategy, TeleporterStrategy,
};

pub fn make_rules(name: &str) -> Result<Box<dyn Rules>, String> {
    match name {
        "Standard" | "" => Ok(Box::new(StandardRules::new())),
        other => Err(format!("unknown rules variant: {other}")),
    }
}

pub fn make_strategy(name: &str) -> Result<Box<dyn Strategy>, String> {
    match name {
        "Random" => Ok(Box::new(RandomStrategy)),
        "Greedy" => Ok(Box::new(GreedyStrategy)),
        "Not Sorry" => Ok(Box::new(NotSorryStrategy)),
        "Survivor" => Ok(Box::new(SurvivorStrategy)),
        "Reverse" => Ok(Box::new(ReverseStrategy)),
        "Teleporter" => Ok(Box::new(TeleporterStrategy)),
        "Sidekick" => Ok(Box::new(SidekickStrategy)),
        "Loser" => Ok(Box::new(LoserStrategy)),
        other => Err(format!("unknown strategy: {other}")),
    }
}

pub fn available_rules() -> Vec<String> {
    vec!["Standard".to_string()]
}

pub fn available_strategies() -> Vec<String> {
    vec![
        "Random".to_string(),
        "Greedy".to_string(),
        "Not Sorry".to_string(),
        "Survivor".to_string(),
        "Reverse".to_string(),
        "Teleporter".to_string(),
        "Sidekick".to_string(),
        "Loser".to_string(),
    ]
}
