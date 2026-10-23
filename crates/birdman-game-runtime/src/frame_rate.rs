#[derive(Default)]
pub(super) struct FrameRateOwner {
    start: Option<f64>,
    previous: Option<f64>,
    intervals: u32,
    rate: Option<f64>,
}

impl FrameRateOwner {
    pub(super) fn observe(&mut self, timestamp: f64) {
        if self.previous == Some(timestamp) {
            return;
        }
        if self
            .previous
            .is_none_or(|previous| timestamp < previous || timestamp - previous > 1000.0)
        {
            *self = Self {
                start: Some(timestamp),
                previous: Some(timestamp),
                ..Self::default()
            };
            return;
        }
        self.previous = Some(timestamp);
        self.intervals += 1;
        let elapsed = timestamp - self.start.unwrap();
        if elapsed >= 750.0 {
            self.rate = Some(f64::from(self.intervals) * 1000.0 / elapsed);
            self.start = Some(timestamp);
            self.intervals = 0;
        }
    }

    pub(super) fn label(&self) -> String {
        self.rate
            .map_or_else(|| "FPS —".to_owned(), |rate| format!("FPS {rate:.1}"))
    }
}
