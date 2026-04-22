//! End-to-end test for the interactive WASM bridge. `#[wasm_bindgen]`-tagged
//! functions are plain Rust fns on non-wasm targets, so we exercise them
//! directly through the same JSON contract the frontend will use.

use serde_json::Value;
use sorry_core::GameHistory;
use sorry_wasm::{
    apply_bot_action, create_interactive_game, destroy_interactive_game, get_game_history,
    get_game_state,
};

fn parse(s: &str) -> Value {
    serde_json::from_str(s).expect("bridge returned malformed JSON")
}

fn assert_ok(v: &Value, ctx: &str) {
    assert!(
        v.get("error").is_none(),
        "{ctx}: unexpected error in response: {v}"
    );
}

#[test]
fn drive_full_interactive_game_through_json_api() {
    let config = serde_json::json!({
        "strategy_names": ["Random", "Random", "Random", "Random"],
        "seed": 42_u64,
    })
    .to_string();

    let created = parse(&create_interactive_game(&config));
    assert_ok(&created, "create");
    let game_id = created["game_id"].as_u64().expect("game_id") as u32;
    assert!(game_id > 0, "game_id 0 is reserved");

    // Pump bot turns until GameOver.
    let mut safety = 10_000;
    loop {
        safety -= 1;
        assert!(safety > 0, "interactive game did not terminate");

        let state = parse(&get_game_state(game_id, -1));
        assert_ok(&state, "get_game_state");
        let action_type = state["state"]["action_needed"]["type"]
            .as_str()
            .expect("action_needed.type");
        if action_type == "GameOver" {
            break;
        }

        let step = parse(&apply_bot_action(game_id, "Random"));
        assert_ok(&step, "apply_bot_action");
        assert!(step.get("action").is_some());
        assert!(step.get("state").is_some());
    }

    // History round-trips through strong typing.
    let history_json = get_game_history(game_id);
    let history_value = parse(&history_json);
    assert_ok(&history_value, "get_game_history");
    let history: GameHistory =
        serde_json::from_str(&history_json).expect("history deserializes as GameHistory");
    assert!(
        !history.winners.is_empty() || history.truncated,
        "game ended with no winners and was not truncated"
    );

    // Player-view variant routes through `get_player_view`.
    let view = parse(&get_game_state(game_id, 0));
    assert_ok(&view, "get_game_state(viewer=0)");
    assert_eq!(view["state"]["viewer"], 0);

    let destroyed = parse(&destroy_interactive_game(game_id));
    assert_eq!(destroyed["removed"], Value::Bool(true));
    let destroyed_again = parse(&destroy_interactive_game(game_id));
    assert_eq!(destroyed_again["removed"], Value::Bool(false));
}

#[test]
fn bridge_reports_errors_as_json() {
    let bogus_config = serde_json::json!({
        "strategy_names": ["Random"],
        "seed": 1_u64,
    })
    .to_string();
    let resp = parse(&create_interactive_game(&bogus_config));
    assert!(
        resp.get("error").is_some(),
        "expected error for too-few players, got {resp}"
    );

    let missing = parse(&get_game_state(99_999, -1));
    assert!(missing["error"].as_str().unwrap().contains("game not found"));

    let unknown_strategy = parse(&apply_bot_action(99_999, "NotAStrategy"));
    assert!(
        unknown_strategy["error"]
            .as_str()
            .unwrap()
            .contains("unknown strategy")
    );
}
