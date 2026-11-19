use crate::util::format_bytes;
use serde_json::Value;
use std::collections::BTreeMap;

/// .NET ticks (100 ns) at the Unix epoch.
const DOTNET_TICKS_UNIX_EPOCH: i64 = 621_355_968_000_000_000;
const DOTNET_TICKS_PER_MILLISECOND: i64 = 10_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParsedRecordValue {
    JsonObject(BTreeMap<String, String>),
    Plain(String),
}

pub fn parse_record_value(value: &[u8]) -> ParsedRecordValue {
    if let Ok(json) = serde_json::from_slice::<Value>(value) {
        if let Value::Object(map) = json {
            let mut fields = BTreeMap::new();
            for (key, field_value) in map {
                fields.insert(key, value_to_cell(&field_value));
            }
            return ParsedRecordValue::JsonObject(fields);
        }
    }
    ParsedRecordValue::Plain(format_bytes(value))
}

pub fn value_to_cell(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.clone(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
    }
}

pub fn format_key_as_time(key: &[u8]) -> String {
    if let Some(formatted) = format_key_as_time_from_integer(parse_key_integer(key)) {
        return formatted;
    }
    format_bytes(key)
}

pub fn table_cell_preview(text: &str) -> String {
    const LIMIT: usize = 240;
    if text.chars().count() <= LIMIT {
        return text.to_string();
    }
    let shortened: String = text.chars().take(LIMIT).collect();
    format!("{shortened}…")
}

fn parse_key_integer(key: &[u8]) -> Option<i64> {
    match key.len() {
        8 => {
            let le = i64::from_le_bytes(key.try_into().ok()?);
            let be = i64::from_be_bytes(key.try_into().ok()?);
            pick_plausible_timestamp(le).or_else(|| pick_plausible_timestamp(be))
        }
        4 => {
            let le = u32::from_le_bytes(key.try_into().ok()?);
            let be = u32::from_be_bytes(key.try_into().ok()?);
            pick_plausible_timestamp(i64::from(le)).or_else(|| pick_plausible_timestamp(i64::from(be)))
        }
        _ => {
            let text = std::str::from_utf8(key).ok()?.trim();
            if text.is_empty() {
                return None;
            }
            if let Ok(value) = text.parse::<i64>() {
                return Some(value);
            }
            if let Ok(value) = text.parse::<u64>() {
                return i64::try_from(value).ok();
            }
            if let Some(suffix) = text.rsplit(['/', ':', '\\']).next() {
                if let Ok(value) = suffix.parse::<i64>() {
                    return Some(value);
                }
            }
            None
        }
    }
}

fn pick_plausible_timestamp(value: i64) -> Option<i64> {
    if value <= 0 {
        return None;
    }
    if looks_like_dotnet_ticks(value) {
        return Some(value);
    }
    if looks_like_unix_millis(value) {
        return Some(value);
    }
    if looks_like_unix_seconds(value) {
        return Some(value);
    }
    None
}

fn looks_like_dotnet_ticks(value: i64) -> bool {
    value >= DOTNET_TICKS_UNIX_EPOCH
}

fn looks_like_unix_millis(value: i64) -> bool {
    (946_684_800_000..=4_102_444_800_000).contains(&value)
}

fn looks_like_unix_seconds(value: i64) -> bool {
    (946_684_800..=4_102_444_800).contains(&value)
}

fn format_key_as_time_from_integer(value: Option<i64>) -> Option<String> {
    let value = value?;
    let millis = if looks_like_dotnet_ticks(value) {
        (value - DOTNET_TICKS_UNIX_EPOCH) / DOTNET_TICKS_PER_MILLISECOND
    } else if looks_like_unix_seconds(value) {
        value * 1000
    } else if looks_like_unix_millis(value) {
        value
    } else {
        return None;
    };
    format_unix_millis_utc(millis)
}

fn format_unix_millis_utc(millis: i64) -> Option<String> {
    const MILLIS_PER_SECOND: i64 = 1000;
    const SECONDS_PER_MINUTE: i64 = 60;
    const MINUTES_PER_HOUR: i64 = 60;
    const HOURS_PER_DAY: i64 = 24;
    if millis < 0 {
        return None;
    }

    let mut remaining = millis;
    let mut year = 1970i64;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        let millis_in_year = days_in_year
            * HOURS_PER_DAY
            * MINUTES_PER_HOUR
            * SECONDS_PER_MINUTE
            * MILLIS_PER_SECOND;
        if remaining < millis_in_year {
            break;
        }
        remaining -= millis_in_year;
        year += 1;
    }

    let mut day_of_year = remaining
        / (HOURS_PER_DAY * MINUTES_PER_HOUR * SECONDS_PER_MINUTE * MILLIS_PER_SECOND);
    remaining %= HOURS_PER_DAY * MINUTES_PER_HOUR * SECONDS_PER_MINUTE * MILLIS_PER_SECOND;

    let month_lengths = month_lengths(year);
    let mut month = 1usize;
    for &length in &month_lengths {
        if day_of_year < i64::from(length) {
            break;
        }
        day_of_year -= i64::from(length);
        month += 1;
    }
    let day = day_of_year + 1;

    let hour = remaining / (MINUTES_PER_HOUR * SECONDS_PER_MINUTE * MILLIS_PER_SECOND);
    remaining %= MINUTES_PER_HOUR * SECONDS_PER_MINUTE * MILLIS_PER_SECOND;
    let minute = remaining / (SECONDS_PER_MINUTE * MILLIS_PER_SECOND);
    remaining %= SECONDS_PER_MINUTE * MILLIS_PER_SECOND;
    let second = remaining / MILLIS_PER_SECOND;
    let subsecond = remaining % MILLIS_PER_SECOND;

    Some(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{subsecond:03}Z"
    ))
}

fn is_leap_year(year: i64) -> bool {
    year % 400 == 0 || (year % 4 == 0 && year % 100 != 0)
}

fn month_lengths(year: i64) -> [u32; 12] {
    [
        31,
        if is_leap_year(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_unix_millisecond_keys() {
        let formatted = format_key_as_time(b"1700000000000");
        assert_eq!(formatted, "2023-11-14T22:13:20.000Z");
    }

    #[test]
    fn formats_eight_byte_little_endian_keys() {
        let key = 1_700_000_000_000i64.to_le_bytes();
        assert_eq!(format_key_as_time(&key), "2023-11-14T22:13:20.000Z");
    }

    #[test]
    fn falls_back_to_text_for_non_time_keys() {
        assert_eq!(format_key_as_time(b"records/00000001"), "records/00000001");
    }
}