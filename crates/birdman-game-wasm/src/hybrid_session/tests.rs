use birdman_game_core::SessionSnapshot;

#[test]
fn additive_hybrid_preparation_preserves_existing_public_factory_and_legacy_layout() {
    let mut bridge = crate::GameSessionBridge::new(0).unwrap();
    bridge.open_setup().unwrap();
    bridge.prepare().unwrap();
    bridge.mark_briefing_ready().unwrap();
    bridge.start_countdown(1).unwrap();
    bridge.advance_countdown().unwrap();
    let snapshot = bridge.launch().unwrap();
    assert_eq!(snapshot.len(), crate::SNAPSHOT_LENGTH);
    assert_eq!(
        bridge
            .session
            .configuration_identity()
            .unwrap()
            .catalog_version,
        1
    );
    assert!(matches!(
        bridge.session.snapshot(),
        SessionSnapshot::FlightRunning { .. }
    ));
}
