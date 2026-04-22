//! Rules and Strategy factories. Mirrors `sorry-wasm/src/lib.rs:27-56` so
//! the set of rulesets/strategies exposed to networked clients stays in sync
//! with the WASM bridge. Add a new variant by editing both.

use sorry_core::{RandomStrategy, Rules, StandardRules, Strategy};

pub fn make_rules(name: &str) -> Result<Box<dyn Rules>, String> {
    match name {
        "Standard" | "" => Ok(Box::new(StandardRules::new())),
        other => Err(format!("unknown rules variant: {other}")),
    }
}

pub fn make_strategy(name: &str) -> Result<Box<dyn Strategy>, String> {
    match name {
        "Random" => Ok(Box::new(RandomStrategy)),
        other => Err(format!("unknown strategy: {other}")),
    }
}

pub fn available_rules() -> Vec<String> {
    vec!["Standard".to_string()]
}

pub fn available_strategies() -> Vec<String> {
    vec!["Random".to_string()]
}
