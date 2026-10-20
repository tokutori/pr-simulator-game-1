use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

/// Converts a platform's saved-file timestamp into the shared RFC 3339 input.
pub fn game_ui_saved_record_timestamp(unix_seconds: i64) -> Option<String> {
    OffsetDateTime::from_unix_timestamp(unix_seconds)
        .ok()?
        .format(&Rfc3339)
        .ok()
}

pub(super) fn saved_at_label(timestamp: &str) -> String {
    let Some(time) = OffsetDateTime::parse(timestamp, &Rfc3339)
        .ok()
        .and_then(|time| time.checked_to_offset(UtcOffset::UTC))
    else {
        return "日時不明".to_owned();
    };
    format!(
        "{:04}/{:02}/{:02} {:02}:{:02} UTC",
        time.year(),
        u8::from(time.month()),
        time.day(),
        time.hour(),
        time.minute()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_dates_use_one_utc_projection_without_changing_the_input() {
        let timestamp = "2026-01-01T00:20:45+09:00";
        assert_eq!(saved_at_label(timestamp), "2025/12/31 15:20 UTC");
        assert_eq!(timestamp, "2026-01-01T00:20:45+09:00");
        assert_eq!(
            saved_at_label("2024-02-29T03:04:05.123Z"),
            "2024/02/29 03:04 UTC"
        );
        assert_eq!(saved_at_label("not a timestamp"), "日時不明");
        assert_eq!(
            game_ui_saved_record_timestamp(0).as_deref(),
            Some("1970-01-01T00:00:00Z")
        );
        assert_eq!(game_ui_saved_record_timestamp(i64::MAX), None);
    }
}
