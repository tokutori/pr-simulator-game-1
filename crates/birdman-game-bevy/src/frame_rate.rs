use bevy::ecs as bevy_ecs;
use bevy::prelude::*;
use std::time::Duration;

const SAMPLE_WINDOW: Duration = Duration::from_millis(750);
const MAX_FRAME_GAP: Duration = Duration::from_secs(1);

#[derive(Default, Resource)]
pub(crate) struct FrameRate {
    window_start: Option<Duration>,
    previous_timestamp: Option<Duration>,
    intervals: u32,
    frames_per_second: Option<f64>,
}

impl FrameRate {
    fn observe(&mut self, timestamp: Duration) {
        if self.previous_timestamp == Some(timestamp) {
            return;
        }
        let restart = match self.previous_timestamp {
            Some(previous) => timestamp < previous || timestamp - previous > MAX_FRAME_GAP,
            None => true,
        };
        if restart {
            *self = Self {
                window_start: Some(timestamp),
                previous_timestamp: Some(timestamp),
                ..default()
            };
            return;
        }
        self.previous_timestamp = Some(timestamp);
        self.intervals += 1;
        let elapsed = timestamp - self.window_start.unwrap_or(timestamp);
        if elapsed < SAMPLE_WINDOW {
            return;
        }
        self.frames_per_second = Some(f64::from(self.intervals) / elapsed.as_secs_f64());
        self.window_start = Some(timestamp);
        self.intervals = 0;
    }

    pub(crate) fn label(&self) -> String {
        match self.frames_per_second {
            Some(value) => format!("FPS {value:.1}"),
            None => "FPS —".into(),
        }
    }
}

pub(crate) fn observe_frame(time: Res<Time<Real>>, mut rate: ResMut<FrameRate>) {
    rate.observe(time.elapsed());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_render_frames_in_a_time_window_not_the_fixed_physics_rate() {
        for frames in [30, 60, 80, 120] {
            let mut rate = FrameRate::default();
            for frame in 0..=frames {
                rate.observe(Duration::from_secs_f64(
                    f64::from(frame) / f64::from(frames),
                ));
            }
            assert!((rate.frames_per_second.unwrap() - f64::from(frames)).abs() < 1e-6);
            assert_eq!(rate.label(), format!("FPS {frames}.0"));
        }
    }

    #[test]
    fn startup_duplicate_frames_and_suspended_time_are_not_fake_fps() {
        let mut rate = FrameRate::default();
        rate.observe(Duration::ZERO);
        rate.observe(Duration::ZERO);
        assert_eq!(rate.intervals, 0);
        assert_eq!(rate.label(), "FPS —");
        rate.observe(Duration::from_millis(750));
        assert_eq!(rate.label(), "FPS 1.3");
        rate.observe(Duration::from_secs(5));
        assert_eq!(rate.label(), "FPS —");
        rate.observe(Duration::from_millis(100));
        assert_eq!(rate.label(), "FPS —");
    }

    #[test]
    fn averages_variable_frame_intervals_by_elapsed_wall_time() {
        let mut rate = FrameRate::default();
        for millis in [0, 100, 150, 250, 750] {
            rate.observe(Duration::from_millis(millis));
        }
        assert!((rate.frames_per_second.unwrap() - 4.0 / 0.75).abs() < 1e-12);
    }
}
