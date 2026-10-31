use bevy::ecs as bevy_ecs;
use bevy::{
    input::gamepad::{GamepadAxis, GamepadConnection, GamepadConnectionEvent, RawGamepadEvent},
    prelude::*,
};
use birdman_game_app::flight_input::FlightDeviceIdentity;
use birdman_game_runtime::ApplicationGamepadObservation;
use std::collections::BTreeMap;

#[derive(Resource, Default)]
pub(crate) struct FlightDevices {
    connections: BTreeMap<u64, ApplicationGamepadObservation>,
    generation: u32,
    touch_observed: bool,
    observations: Vec<ApplicationGamepadObservation>,
}

impl FlightDevices {
    pub(crate) fn touch_available(&self) -> bool {
        self.touch_observed
    }

    pub(crate) fn observations(&self) -> &[ApplicationGamepadObservation] {
        &self.observations
    }

    fn connection(&mut self, event: &GamepadConnectionEvent) -> Result<(), &'static str> {
        let slot = event.gamepad.to_bits();
        match event.connection {
            GamepadConnection::Connected { .. } => {
                self.generation = self
                    .generation
                    .checked_add(1)
                    .ok_or("Gamepad接続世代の上限")?;
                self.connections.insert(
                    slot,
                    ApplicationGamepadObservation {
                        identity: FlightDeviceIdentity {
                            slot_low: slot as u32,
                            slot_high: (slot >> 32) as u32,
                            generation: self.generation,
                        },
                        connected: true,
                        mapping: "standard-y-up".into(),
                        axes: vec![Some(0.0); 4],
                        buttons: vec![false; 2],
                    },
                );
            }
            GamepadConnection::Disconnected => {
                self.connections.remove(&slot);
            }
        }
        Ok(())
    }

    fn event(&mut self, event: &RawGamepadEvent) -> Result<(), &'static str> {
        match event {
            RawGamepadEvent::Connection(event) => self.connection(event)?,
            RawGamepadEvent::Axis(event) => {
                let index = match event.axis {
                    GamepadAxis::LeftStickX => 0,
                    GamepadAxis::LeftStickY => 1,
                    GamepadAxis::RightStickX => 2,
                    GamepadAxis::RightStickY => 3,
                    _ => return Ok(()),
                };
                if let Some(pad) = self.connections.get_mut(&event.gamepad.to_bits()) {
                    pad.axes[index] = event.value.is_finite().then_some(f64::from(event.value));
                }
            }
            RawGamepadEvent::Button(_) => {}
        }
        Ok(())
    }
}

pub(crate) fn observe_devices(
    mut devices: ResMut<FlightDevices>,
    mut raw: MessageReader<RawGamepadEvent>,
    mut touch: MessageReader<bevy::input::touch::TouchInput>,
    mut connections: MessageReader<GamepadConnectionEvent>,
    gamepads: Query<&Gamepad>,
) {
    devices.touch_observed = touch.read().count() != 0;
    let changed = !raw.is_empty() || !connections.is_empty();
    let raw = raw.read().collect::<Vec<_>>();
    let result = (|| {
        for connection in connections.read() {
            if !raw.iter().any(|event| {
                matches!(event,
                RawGamepadEvent::Connection(candidate) if candidate == connection)
            }) {
                devices.connection(connection)?;
            }
        }
        for event in &raw {
            devices.event(event)?;
        }
        Ok::<_, &'static str>(())
    })();
    if let Err(error) = result {
        devices.connections.clear();
        error!("Gamepad input stream: {error}");
    }
    if changed {
        let observed = devices.connections.values().cloned().collect::<Vec<_>>();
        devices.observations = observed;
    }
    for pad in &mut devices.observations {
        let bits = (u64::from(pad.identity.slot_high) << 32) | u64::from(pad.identity.slot_low);
        if let Ok(gamepad) = gamepads.get(Entity::from_bits(bits)) {
            pad.buttons[0] = gamepad.pressed(bevy::input::gamepad::GamepadButton::South);
            pad.buttons[1] = gamepad.pressed(bevy::input::gamepad::GamepadButton::East);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::gamepad::RawGamepadAxisChangedEvent;

    fn connected(gamepad: Entity) -> GamepadConnectionEvent {
        GamepadConnectionEvent::new(
            gamepad,
            GamepadConnection::Connected {
                name: "test".into(),
                vendor_id: None,
                product_id: None,
            },
        )
    }

    #[test]
    fn ordered_raw_axes_keep_small_values_and_retire_the_old_connection() {
        let mut devices = FlightDevices::default();
        let mut world = World::new();
        let gamepad = world.spawn_empty().id();
        devices.connection(&connected(gamepad)).unwrap();
        let initial = devices.connections[&gamepad.to_bits()].clone();
        for (axis, value, index) in [
            (GamepadAxis::LeftStickY, 0.02, 1),
            (GamepadAxis::LeftStickX, -0.03, 0),
            (GamepadAxis::RightStickY, 0.5, 3),
        ] {
            devices
                .event(&RawGamepadAxisChangedEvent::new(gamepad, axis, value).into())
                .unwrap();
            assert_eq!(
                devices.connections[&gamepad.to_bits()].axes[index],
                Some(f64::from(value))
            );
        }
        devices
            .event(&GamepadConnectionEvent::new(gamepad, GamepadConnection::Disconnected).into())
            .unwrap();
        devices
            .event(&RawGamepadAxisChangedEvent::new(gamepad, GamepadAxis::LeftStickX, 1.0).into())
            .unwrap();
        devices.event(&connected(gamepad).into()).unwrap();
        let reconnected = devices.connections[&gamepad.to_bits()].clone();
        assert_ne!(initial.identity, reconnected.identity);
        assert_eq!(reconnected.axes, vec![Some(0.0); 4]);
        devices
            .event(
                &RawGamepadAxisChangedEvent::new(gamepad, GamepadAxis::LeftStickY, f32::NAN).into(),
            )
            .unwrap();
        assert_eq!(devices.connections[&gamepad.to_bits()].axes[1], None);
    }

    #[test]
    fn startup_and_regular_connection_streams_register_each_connection_once() {
        let mut app = App::new();
        app.add_plugins(bevy::input::InputPlugin)
            .init_resource::<FlightDevices>()
            .add_systems(PreUpdate, observe_devices.after(bevy::input::InputSystems));
        let gamepad = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(connected(gamepad));
        app.update();
        let initial = app.world().resource::<FlightDevices>().observations()[0].clone();
        app.world_mut().write_message(GamepadConnectionEvent::new(
            gamepad,
            GamepadConnection::Disconnected,
        ));
        app.world_mut()
            .write_message(RawGamepadEvent::Connection(GamepadConnectionEvent::new(
                gamepad,
                GamepadConnection::Disconnected,
            )));
        app.world_mut().write_message(connected(gamepad));
        app.world_mut()
            .write_message(RawGamepadEvent::Connection(connected(gamepad)));
        app.world_mut()
            .write_message(RawGamepadEvent::Axis(RawGamepadAxisChangedEvent::new(
                gamepad,
                GamepadAxis::LeftStickY,
                0.75,
            )));
        app.update();
        let observed = app.world().resource::<FlightDevices>().observations();
        assert_eq!(observed.len(), 1);
        assert_eq!(
            observed[0].identity.generation,
            initial.identity.generation + 1
        );
        assert_eq!(
            observed[0].axes,
            vec![Some(0.0), Some(0.75), Some(0.0), Some(0.0)]
        );
        // The adapter reports only the current batch; the aggregate runtime retains availability.
        assert!(!app.world().resource::<FlightDevices>().touch_available());
        app.world_mut()
            .write_message(bevy::input::touch::TouchInput {
                phase: bevy::input::touch::TouchPhase::Started,
                position: Vec2::ZERO,
                window: gamepad,
                force: None,
                id: 1,
            });
        app.update();
        assert!(app.world().resource::<FlightDevices>().touch_available());
        app.update();
        assert!(!app.world().resource::<FlightDevices>().touch_available());
    }
}
