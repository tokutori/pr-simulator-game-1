use bevy::ecs as bevy_ecs;
use bevy::{
    input::gamepad::{GamepadAxis, GamepadConnection, GamepadConnectionEvent, RawGamepadEvent},
    prelude::*,
};
use birdman_game_app::flight_input::{FlightDeviceIdentity, FlightGamepadObservation};
use std::collections::BTreeMap;

#[derive(Resource, Default)]
pub(crate) struct FlightDevices {
    connections: BTreeMap<u64, FlightGamepadObservation>,
    generation: u32,
    observations: Vec<FlightGamepadObservation>,
    #[cfg(feature = "verify-harness")]
    verification: Option<[FlightGamepadObservation; 1]>,
}

impl FlightDevices {
    pub(crate) fn observations(&self) -> &[FlightGamepadObservation] {
        #[cfg(feature = "verify-harness")]
        if let Some(observations) = &self.verification {
            return observations;
        }
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
                    FlightGamepadObservation {
                        identity: FlightDeviceIdentity {
                            slot_low: slot as u32,
                            slot_high: (slot >> 32) as u32,
                            generation: self.generation,
                        },
                        connected: true,
                        axes: [Some(0.0); 3],
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
                    GamepadAxis::LeftStickY => 0,
                    GamepadAxis::LeftStickX => 1,
                    GamepadAxis::RightStickY => 2,
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

    #[cfg(feature = "verify-harness")]
    pub(crate) fn verification_axes(&mut self, axes: [f64; 3]) {
        self.verification = Some([FlightGamepadObservation {
            identity: FlightDeviceIdentity {
                slot_low: 0,
                slot_high: 0,
                generation: 1,
            },
            connected: true,
            axes: axes.map(Some),
        }]);
    }
}

pub(crate) fn observe_devices(
    mut devices: ResMut<FlightDevices>,
    mut raw: MessageReader<RawGamepadEvent>,
    mut connections: MessageReader<GamepadConnectionEvent>,
    mut session: ResMut<super::native_session::NativeSession>,
) {
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
        session.game.fail_flight_adapter();
        session.notice = Some(error.into());
    }
    if changed {
        let observed = devices.connections.values().copied().collect::<Vec<_>>();
        devices.observations = observed;
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
        let initial = devices.connections[&gamepad.to_bits()];
        for (axis, value, index) in [
            (GamepadAxis::LeftStickY, 0.02, 0),
            (GamepadAxis::LeftStickX, -0.03, 1),
            (GamepadAxis::RightStickY, 0.5, 2),
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
        let reconnected = devices.connections[&gamepad.to_bits()];
        assert_ne!(initial.identity, reconnected.identity);
        assert_eq!(reconnected.axes, [Some(0.0); 3]);
        devices
            .event(
                &RawGamepadAxisChangedEvent::new(gamepad, GamepadAxis::LeftStickY, f32::NAN).into(),
            )
            .unwrap();
        assert_eq!(devices.connections[&gamepad.to_bits()].axes[0], None);
    }

    #[test]
    fn startup_and_regular_connection_streams_register_each_connection_once() {
        let mut app = App::new();
        app.add_plugins(bevy::input::InputPlugin)
            .init_resource::<FlightDevices>()
            .init_resource::<super::super::native_session::NativeSession>()
            .add_systems(PreUpdate, observe_devices.after(bevy::input::InputSystems));
        let gamepad = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(connected(gamepad));
        app.update();
        let initial = app.world().resource::<FlightDevices>().observations()[0];
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
        assert_eq!(observed[0].axes, [Some(0.75), Some(0.0), Some(0.0)]);
    }
}
