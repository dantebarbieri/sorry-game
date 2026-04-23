//! wasm-bindgen wrapper around `sorry-core`.
//!
//! The boundary is intentionally narrow: every export takes a JSON string
//! (or nothing) and returns a JSON string. Errors are serialized as
//! `{"error": "..."}` rather than thrown, so the JS side always receives
//! well-formed JSON. No `serde-wasm-bindgen`, no complex `JsValue` shuttling.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use sorry_core::{
    AggregateStats, BoardGeometry, Game, GameHistory, GameStats, InteractiveGame,
    InteractiveGameState, PlayerAction, PlayerId, RandomStrategy, Rules, Simulator,
    SimulatorConfig, StandardRules, Strategy, StrategyDescription,
};

// ─── Registries ───────────────────────────────────────────────────────
//
// Name → concrete type. Both a factory form (for `Simulator`, which
// builds a fresh instance per game) and a direct form (for one-off
// games) are provided. Add new variants/strategies by extending these
// match statements.

fn make_rules(name: &str) -> Result<Box<dyn Rules>, String> {
    match name {
        "Standard" | "" => Ok(Box::new(StandardRules::new())),
        "PlayOut" => Ok(Box::new(StandardRules::new_play_out())),
        other => Err(format!("unknown rules variant: {other}")),
    }
}

fn make_rules_factory(name: &str) -> Result<Box<dyn Fn() -> Box<dyn Rules>>, String> {
    match name {
        "Standard" | "" => Ok(Box::new(|| Box::new(StandardRules::new()))),
        "PlayOut" => Ok(Box::new(|| Box::new(StandardRules::new_play_out()))),
        other => Err(format!("unknown rules variant: {other}")),
    }
}

fn make_strategy(name: &str) -> Result<Box<dyn Strategy>, String> {
    match name {
        "Random" => Ok(Box::new(RandomStrategy)),
        other => Err(format!("unknown strategy: {other}")),
    }
}

fn make_strategy_factory(name: &str) -> Result<Box<dyn Fn() -> Box<dyn Strategy>>, String> {
    match name {
        "Random" => Ok(Box::new(|| Box::new(RandomStrategy))),
        other => Err(format!("unknown strategy: {other}")),
    }
}

const AVAILABLE_RULES: &[&str] = &["Standard", "PlayOut"];
const AVAILABLE_STRATEGIES: &[&str] = &["Random"];

// ─── DTOs ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct WasmSimConfig {
    num_games: usize,
    base_seed: u64,
    strategies: Vec<String>,
    #[serde(default)]
    rules: Option<String>,
}

#[derive(Deserialize)]
struct SingleGameConfig {
    seed: u64,
    strategies: Vec<String>,
    #[serde(default)]
    rules: Option<String>,
    #[serde(default)]
    max_turns: Option<usize>,
}

#[derive(Serialize)]
struct SimWithHistories {
    stats: AggregateStats,
    histories: Vec<GameHistory>,
}

#[derive(Serialize)]
struct SingleGameResult {
    stats: GameStats,
    history: GameHistory,
}

#[derive(Serialize)]
struct RulesInfo {
    name: String,
    min_players: usize,
    max_players: usize,
    pawns_per_player: usize,
    hand_size: usize,
    deck_size: usize,
    num_spaces: usize,
    reshuffle_on_empty_deck: bool,
}

#[derive(Deserialize)]
struct CreateInteractiveConfig {
    strategy_names: Vec<String>,
    seed: u64,
    #[serde(default)]
    rules: Option<String>,
    #[serde(default)]
    max_turns: Option<usize>,
}

#[derive(Serialize)]
struct CreateInteractiveResponse {
    game_id: u32,
    state: InteractiveGameState,
}

#[derive(Serialize)]
struct StateResponse<T: Serialize> {
    state: T,
}

#[derive(Serialize)]
struct BotActionResponse {
    action: PlayerAction,
    state: InteractiveGameState,
}

// ─── Error wrapper ────────────────────────────────────────────────────

fn to_json_or_error<F, T>(f: F) -> String
where
    F: FnOnce() -> Result<T, String>,
    T: serde::Serialize,
{
    match f() {
        Ok(val) => serde_json::to_string(&val)
            .unwrap_or_else(|e| serde_json::json!({ "error": format!("serialize: {e}") }).to_string()),
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    }
}

fn build_simulator(cfg: &WasmSimConfig) -> Result<Simulator, String> {
    let rules_name = cfg.rules.as_deref().unwrap_or("Standard");
    let rules_factory = make_rules_factory(rules_name)?;
    let strategy_factories = cfg
        .strategies
        .iter()
        .map(|s| make_strategy_factory(s))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Simulator::new(
        SimulatorConfig {
            num_games: cfg.num_games,
            base_seed: cfg.base_seed,
        },
        rules_factory,
        strategy_factories,
    ))
}

fn run_single_game(cfg: &SingleGameConfig) -> Result<GameHistory, String> {
    let rules_name = cfg.rules.as_deref().unwrap_or("Standard");
    let rules = make_rules(rules_name)?;
    let strategies = cfg
        .strategies
        .iter()
        .map(|s| make_strategy(s))
        .collect::<Result<Vec<_>, _>>()?;
    let mut game = Game::new(rules, strategies, cfg.seed).map_err(|e| e.to_string())?;
    if let Some(limit) = cfg.max_turns {
        game.set_max_turns(limit);
    }
    game.play().map_err(|e| e.to_string())
}

// ─── #[wasm_bindgen] exports ──────────────────────────────────────────

#[wasm_bindgen]
pub fn simulate(config_json: &str) -> String {
    to_json_or_error(|| {
        let cfg: WasmSimConfig =
            serde_json::from_str(config_json).map_err(|e| format!("invalid config: {e}"))?;
        let sim = build_simulator(&cfg)?;
        sim.run_stats_only().map_err(|e| e.to_string())
    })
}

#[wasm_bindgen]
pub fn simulate_with_histories(config_json: &str) -> String {
    to_json_or_error(|| {
        let cfg: WasmSimConfig =
            serde_json::from_str(config_json).map_err(|e| format!("invalid config: {e}"))?;
        let sim = build_simulator(&cfg)?;
        let (histories, stats) = sim.run().map_err(|e| e.to_string())?;
        Ok(SimWithHistories { stats, histories })
    })
}

#[wasm_bindgen]
pub fn simulate_one(config_json: &str) -> String {
    to_json_or_error(|| {
        let cfg: SingleGameConfig =
            serde_json::from_str(config_json).map_err(|e| format!("invalid config: {e}"))?;
        let history = run_single_game(&cfg)?;
        Ok(GameStats::from_history(&history))
    })
}

#[wasm_bindgen]
pub fn simulate_one_with_history(config_json: &str) -> String {
    to_json_or_error(|| {
        let cfg: SingleGameConfig =
            serde_json::from_str(config_json).map_err(|e| format!("invalid config: {e}"))?;
        let history = run_single_game(&cfg)?;
        let stats = GameStats::from_history(&history);
        Ok(SingleGameResult { stats, history })
    })
}

#[wasm_bindgen]
pub fn get_available_rules() -> String {
    to_json_or_error(|| Ok::<_, String>(AVAILABLE_RULES))
}

#[wasm_bindgen]
pub fn get_available_strategies() -> String {
    to_json_or_error(|| Ok::<_, String>(AVAILABLE_STRATEGIES))
}

#[wasm_bindgen]
pub fn get_strategy_descriptions() -> String {
    to_json_or_error(|| {
        let descriptions: Vec<StrategyDescription> = AVAILABLE_STRATEGIES
            .iter()
            .map(|name| make_strategy(name).map(|s| s.describe()))
            .collect::<Result<_, _>>()?;
        Ok(descriptions)
    })
}

#[wasm_bindgen]
pub fn get_rules_info(rules_name: &str) -> String {
    to_json_or_error(|| {
        let rules = make_rules(rules_name)?;
        Ok(RulesInfo {
            name: rules.name().to_string(),
            min_players: rules.min_players(),
            max_players: rules.max_players(),
            pawns_per_player: rules.pawns_per_player(),
            hand_size: rules.hand_size(),
            deck_size: rules.build_deck().len(),
            num_spaces: rules.spaces().len(),
            reshuffle_on_empty_deck: rules.reshuffle_on_empty_deck(),
        })
    })
}

#[wasm_bindgen]
pub fn get_board_geometry(rules_name: &str) -> String {
    to_json_or_error(|| {
        let rules = make_rules(rules_name)?;
        Ok::<BoardGeometry, String>(rules.board_geometry())
    })
}

// ─── Interactive-mode registry ────────────────────────────────────────
//
// Games live in a thread-local map for the lifetime of the WASM instance.
// The frontend is responsible for calling `destroy_interactive_game` when a
// session ends. `game_id` 0 is reserved so JS can treat it as "no game".

thread_local! {
    static INTERACTIVE_GAMES: RefCell<HashMap<u32, InteractiveGame>> =
        RefCell::new(HashMap::new());
    static NEXT_GAME_ID: Cell<u32> = const { Cell::new(1) };
}

fn next_game_id() -> u32 {
    NEXT_GAME_ID.with(|c| {
        let id = c.get();
        c.set(id.wrapping_add(1).max(1));
        id
    })
}

fn with_game<R>(game_id: u32, f: impl FnOnce(&mut InteractiveGame) -> Result<R, String>)
-> Result<R, String> {
    INTERACTIVE_GAMES.with(|games| {
        let mut games = games.borrow_mut();
        let game = games
            .get_mut(&game_id)
            .ok_or_else(|| format!("game not found: {game_id}"))?;
        f(game)
    })
}

// ─── Interactive-mode exports ─────────────────────────────────────────

#[wasm_bindgen]
pub fn create_interactive_game(config_json: &str) -> String {
    to_json_or_error(|| {
        let cfg: CreateInteractiveConfig = serde_json::from_str(config_json)
            .map_err(|e| format!("invalid config: {e}"))?;
        let rules_name = cfg.rules.as_deref().unwrap_or("Standard");
        let rules = make_rules(rules_name)?;
        let mut game =
            InteractiveGame::new_with_strategy_names(rules, cfg.strategy_names, cfg.seed)
                .map_err(|e| e.to_string())?;
        if let Some(limit) = cfg.max_turns {
            game.set_max_turns(limit);
        }
        let state = game.get_full_state();
        let id = next_game_id();
        INTERACTIVE_GAMES.with(|games| {
            games.borrow_mut().insert(id, game);
        });
        Ok(CreateInteractiveResponse { game_id: id, state })
    })
}

#[wasm_bindgen]
pub fn get_game_state(game_id: u32, viewer: i32) -> String {
    to_json_or_error(|| {
        INTERACTIVE_GAMES.with(|games| {
            let games = games.borrow();
            let game = games
                .get(&game_id)
                .ok_or_else(|| format!("game not found: {game_id}"))?;
            if viewer < 0 {
                Ok(serde_json::to_value(StateResponse {
                    state: game.get_full_state(),
                })
                .map_err(|e| format!("serialize: {e}"))?)
            } else {
                let viewer_id = PlayerId(viewer as u8);
                Ok(serde_json::to_value(StateResponse {
                    state: game.get_player_view(viewer_id),
                })
                .map_err(|e| format!("serialize: {e}"))?)
            }
        })
    })
}

#[wasm_bindgen]
pub fn get_game_history(game_id: u32) -> String {
    to_json_or_error(|| {
        INTERACTIVE_GAMES.with(|games| {
            let games = games.borrow();
            let game = games
                .get(&game_id)
                .ok_or_else(|| format!("game not found: {game_id}"))?;
            Ok(game.history().clone())
        })
    })
}

#[wasm_bindgen]
pub fn apply_action(game_id: u32, action_json: &str) -> String {
    to_json_or_error(|| {
        let action: PlayerAction = serde_json::from_str(action_json)
            .map_err(|e| format!("invalid action: {e}"))?;
        with_game(game_id, |game| {
            game.apply_action(action).map_err(|e| e.to_string())?;
            Ok(StateResponse {
                state: game.get_full_state(),
            })
        })
    })
}

#[wasm_bindgen]
pub fn apply_bot_action(game_id: u32, strategy_name: &str) -> String {
    to_json_or_error(|| {
        let strategy = make_strategy(strategy_name)?;
        with_game(game_id, |game| {
            let action = game
                .get_bot_action(&*strategy)
                .map_err(|e| e.to_string())?;
            game.apply_action(action.clone())
                .map_err(|e| e.to_string())?;
            Ok(BotActionResponse {
                action,
                state: game.get_full_state(),
            })
        })
    })
}

#[wasm_bindgen]
pub fn destroy_interactive_game(game_id: u32) -> String {
    let removed = INTERACTIVE_GAMES.with(|games| games.borrow_mut().remove(&game_id).is_some());
    serde_json::json!({ "removed": removed }).to_string()
}
