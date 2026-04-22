//! End-to-end WebSocket smoke test: spawns the server on an ephemeral port,
//! drives a 2-player game with a real WS client, and asserts the wire
//! protocol matches what the frontend will see.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use sorry_server::{AppStateInner, build_app, lobby::Lobby};
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

async fn spawn_server() -> SocketAddr {
    let app_state = Arc::new(AppStateInner {
        lobby: Lobby::new(10),
    });
    let app = build_app(app_state, None);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });

    // Tiny pause so the listener is ready before clients connect.
    tokio::time::sleep(Duration::from_millis(20)).await;
    addr
}

async fn recv_json(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Value {
    let timeout = tokio::time::Duration::from_secs(5);
    let msg = tokio::time::timeout(timeout, ws.next())
        .await
        .expect("ws message timeout")
        .expect("stream closed")
        .expect("ws error");
    match msg {
        Message::Text(text) => serde_json::from_str(&text).expect("invalid json"),
        other => panic!("unexpected non-text frame: {other:?}"),
    }
}

async fn recv_until(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    target_type: &str,
) -> Value {
    loop {
        let v = recv_json(ws).await;
        if v["type"] == target_type {
            return v;
        }
    }
}

#[tokio::test]
async fn two_player_game_end_to_end() {
    let addr = spawn_server().await;
    let base = format!("http://{addr}/api");
    let client = reqwest::Client::new();

    // 1. Creator creates a room.
    let create: Value = client
        .post(format!("{base}/rooms"))
        .json(&json!({ "player_name": "Alice", "num_players": 2, "rules": "Standard" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let code = create["room_code"].as_str().unwrap().to_string();
    let token_a = create["session_token"].as_str().unwrap().to_string();

    // 2. Second player joins.
    let join: Value = client
        .post(format!("{base}/rooms/{code}/join"))
        .json(&json!({ "player_name": "Bob" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let token_b = join["session_token"].as_str().unwrap().to_string();
    assert_eq!(join["player_index"], 1);

    // 3. Both players open WS connections.
    let url_a = format!("ws://{addr}/api/rooms/{code}/ws?token={token_a}");
    let url_b = format!("ws://{addr}/api/rooms/{code}/ws?token={token_b}");
    let (mut ws_a, _) = connect_async(&url_a).await.expect("connect a");
    let (mut ws_b, _) = connect_async(&url_b).await.expect("connect b");

    // Both receive a RoomState first (still in lobby phase).
    let first_a = recv_json(&mut ws_a).await;
    let first_b = recv_json(&mut ws_b).await;
    assert_eq!(first_a["type"], "RoomState");
    assert_eq!(first_b["type"], "RoomState");
    assert_eq!(first_a["state"]["phase"], "lobby");

    // Disable the turn timer so bot-turn / timeout tasks don't fire during
    // the test window and create flaky timing.
    ws_a
        .send(Message::Text(
            json!({ "type": "SetTurnTimer", "secs": null }).to_string(),
        ))
        .await
        .unwrap();

    // 4. Creator starts the game.
    ws_a
        .send(Message::Text(json!({ "type": "StartGame" }).to_string()))
        .await
        .unwrap();

    // 5. Both should receive GameState.
    let gs_a = recv_until(&mut ws_a, "GameState").await;
    let gs_b = recv_until(&mut ws_b, "GameState").await;

    // Per-player filtering: each view's `viewer` field matches their slot.
    assert_eq!(gs_a["state"]["viewer"], 0);
    assert_eq!(gs_b["state"]["viewer"], 1);
    assert_eq!(gs_a["state"]["num_players"], 2);

    // 6. The current player plays their first legal move.
    let (mut current_ws, mut other_ws, current_idx) = {
        let current_player = gs_a["state"]["current_player"].as_u64().unwrap();
        if current_player == 0 {
            (ws_a, ws_b, 0u64)
        } else {
            (ws_b, ws_a, 1u64)
        }
    };

    // Re-read current state from the acting player's view so the action
    // list is theirs. We already have gs_a / gs_b at the top of the match,
    // but we shadowed — just use whichever is current.
    let current_state = if current_idx == 0 { &gs_a } else { &gs_b };
    let action_needed = &current_state["state"]["action_needed"];
    assert_eq!(action_needed["type"], "ChooseMove");
    let legal = action_needed["legal_moves"].as_array().expect("legal_moves");
    assert!(!legal.is_empty(), "no legal moves on opening turn");
    let mv = legal[0].clone();

    let action_msg = json!({ "type": "Action", "action": { "type": "PlayMove", "mv": mv } });
    current_ws
        .send(Message::Text(action_msg.to_string()))
        .await
        .unwrap();

    // 7. Both players receive ActionApplied.
    let _ = recv_until(&mut current_ws, "ActionApplied").await;
    let other_applied = recv_until(&mut other_ws, "ActionApplied").await;
    assert_eq!(other_applied["player"], current_idx);

    let _ = current_ws.close(None).await;
    let _ = other_ws.close(None).await;
}

#[tokio::test]
async fn meta_endpoint_lists_rules_and_strategies() {
    let addr = spawn_server().await;
    let client = reqwest::Client::new();
    let meta: Value = client
        .get(format!("http://{addr}/api/meta"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert!(meta["available_rules"].as_array().unwrap().iter().any(|v| v == "Standard"));
    assert!(meta["available_strategies"].as_array().unwrap().iter().any(|v| v == "Random"));
}

#[tokio::test]
async fn ws_rejects_wrong_token() {
    let addr = spawn_server().await;
    let base = format!("http://{addr}/api");
    let client = reqwest::Client::new();

    let create: Value = client
        .post(format!("{base}/rooms"))
        .json(&json!({ "player_name": "Alice", "num_players": 2 }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let code = create["room_code"].as_str().unwrap().to_string();

    let url = format!("ws://{addr}/api/rooms/{code}/ws?token=not-a-real-token");
    let res = connect_async(&url).await;
    assert!(res.is_err(), "expected WS handshake to fail with bad token");
}
