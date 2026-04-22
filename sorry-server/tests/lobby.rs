use std::time::Duration;

use sorry_server::lobby::Lobby;
use sorry_server::messages::PlayerSlotType;
use sorry_server::room::RoomPhase;

#[tokio::test]
async fn create_room_issues_token_and_seats_creator() {
    let lobby = Lobby::new(10);
    let (code, token, idx) = lobby
        .create_room("Alice".into(), 2, None)
        .expect("create_room");

    assert_eq!(idx, 0);
    assert_eq!(code.len(), 6);
    assert_eq!(lobby.rooms.len(), 1);

    let session = lobby.get_session(&token.to_string()).expect("session");
    assert_eq!(session, (code.clone(), 0));

    let room = lobby.get_room(&code).expect("room").clone();
    let room = room.lock().await;
    assert_eq!(room.phase, RoomPhase::Lobby);
    assert_eq!(room.creator, 0);
    assert_eq!(room.players[0].name, "Alice");
    assert!(matches!(room.players[0].slot_type, PlayerSlotType::Human));
    assert!(matches!(room.players[1].slot_type, PlayerSlotType::Empty));
}

#[tokio::test]
async fn join_room_fills_next_empty_slot() {
    let lobby = Lobby::new(10);
    let (code, _, _) = lobby.create_room("Alice".into(), 2, None).unwrap();

    let (token, idx) = lobby.join_room(&code, "Bob".into()).await.unwrap();
    assert_eq!(idx, 1);

    let session = lobby.get_session(&token.to_string()).expect("session");
    assert_eq!(session, (code, 1));
}

#[tokio::test]
async fn join_room_rejects_bad_code() {
    let lobby = Lobby::new(10);
    let err = lobby
        .join_room("NOPE12", "Bob".into())
        .await
        .unwrap_err();
    assert!(err.contains("not found"), "got: {err}");
}

#[tokio::test]
async fn rejects_player_count_outside_range() {
    let lobby = Lobby::new(10);
    assert!(lobby.create_room("A".into(), 1, None).is_err());
    assert!(lobby.create_room("A".into(), 5, None).is_err());
    assert!(lobby.create_room("A".into(), 2, None).is_ok());
    assert!(lobby.create_room("A".into(), 4, None).is_ok());
}

#[tokio::test]
async fn cleanup_removes_idle_lobbies_with_no_players_connected() {
    let lobby = Lobby::new(10);
    let (code, _token, _) = lobby.create_room("Alice".into(), 2, None).unwrap();

    // Nothing connected; a zero-timeout cleanup should reap the room.
    lobby.cleanup_stale_rooms(Duration::from_secs(0), Duration::from_secs(0));
    assert!(lobby.get_room(&code).is_none());
    assert_eq!(lobby.rooms.len(), 0);
    assert_eq!(lobby.sessions.len(), 0);
}
