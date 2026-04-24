use sorry_server::lobby::Lobby;
use sorry_server::messages::PlayerSlotType;

#[tokio::test]
async fn take_slot_moves_human_and_empties_origin() {
    let lobby = Lobby::new(10);
    let (code, _, _) = lobby.create_room("Alice".into(), 4, None).unwrap();
    lobby.join_room(&code, "Bob".into()).await.unwrap();

    let room = lobby.get_room(&code).unwrap();
    let mut guard = room.lock().await;
    // Bob is in slot 1; move to slot 3 (Empty).
    guard.take_slot(1, 3).expect("take_slot");

    assert!(matches!(guard.players[1].slot_type, PlayerSlotType::Empty));
    assert!(matches!(guard.players[3].slot_type, PlayerSlotType::Human));
    assert_eq!(guard.players[3].name, "Bob");
}

#[tokio::test]
async fn take_slot_rejects_occupied_target() {
    let lobby = Lobby::new(10);
    let (code, _, _) = lobby.create_room("Alice".into(), 2, None).unwrap();
    lobby.join_room(&code, "Bob".into()).await.unwrap();

    let room = lobby.get_room(&code).unwrap();
    let mut guard = room.lock().await;
    // Alice in 0, Bob in 1. Bob tries to take Alice's slot.
    let err = guard.take_slot(1, 0).unwrap_err();
    assert!(err.contains("taken"), "got: {err}");
}

#[tokio::test]
async fn take_slot_updates_creator() {
    let lobby = Lobby::new(10);
    let (code, _, _) = lobby.create_room("Alice".into(), 4, None).unwrap();

    let room = lobby.get_room(&code).unwrap();
    let mut guard = room.lock().await;
    assert_eq!(guard.creator, 0);
    guard.take_slot(0, 2).expect("take_slot");
    assert_eq!(guard.creator, 2);
}

#[tokio::test]
async fn become_spectator_empties_slot_and_enlists() {
    let lobby = Lobby::new(10);
    let (code, _, _) = lobby.create_room("Alice".into(), 4, None).unwrap();
    lobby.join_room(&code, "Bob".into()).await.unwrap();

    let room = lobby.get_room(&code).unwrap();
    let mut guard = room.lock().await;
    guard.become_spectator(1).expect("become_spectator");

    assert!(matches!(guard.players[1].slot_type, PlayerSlotType::Empty));
    assert_eq!(guard.spectators.len(), 1);
    assert_eq!(guard.spectators[0].name, "Bob");
}

#[tokio::test]
async fn spectator_take_slot_seats_human() {
    let lobby = Lobby::new(10);
    let (code, _, _) = lobby.create_room("Alice".into(), 4, None).unwrap();
    lobby.spectate_room(&code, "Cara".into()).await.unwrap();

    let room = lobby.get_room(&code).unwrap();
    let mut guard = room.lock().await;
    assert_eq!(guard.spectators.len(), 1);
    let new_slot = guard.spectator_take_slot(0, 2).expect("take");
    assert_eq!(new_slot, 2);
    assert!(matches!(guard.players[2].slot_type, PlayerSlotType::Human));
    assert_eq!(guard.players[2].name, "Cara");
    assert!(guard.spectators.is_empty());
}

#[tokio::test]
async fn swap_via_spectator_flow() {
    // Scenario from the ask: 4 humans; slot 0 wants to swap with slot 3.
    // Slot 0 → spectator; slot 3 → slot 0; spectator → slot 3.
    let lobby = Lobby::new(10);
    let (code, _, _) = lobby.create_room("A".into(), 4, None).unwrap();
    lobby.join_room(&code, "B".into()).await.unwrap();
    lobby.join_room(&code, "C".into()).await.unwrap();
    lobby.join_room(&code, "D".into()).await.unwrap();

    let room = lobby.get_room(&code).unwrap();
    let mut guard = room.lock().await;

    // A becomes spectator.
    guard.become_spectator(0).expect("spec");
    // D moves from slot 3 to slot 0.
    guard.take_slot(3, 0).expect("take");
    // Spectator (A) takes slot 3.
    guard.spectator_take_slot(0, 3).expect("spec-take");

    assert_eq!(guard.players[0].name, "D");
    assert_eq!(guard.players[1].name, "B");
    assert_eq!(guard.players[2].name, "C");
    assert_eq!(guard.players[3].name, "A");
    assert!(guard.spectators.is_empty());
}
