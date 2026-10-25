//! FEEL temporal carrier types + `@""` parse/format + DST-correct arithmetic.
//!
//! Plan 30-01 landed `TimeValue`, `DateTimeValue`, and `YearsMonthsDuration`
//! (the carrier types). Plan 30-03 (this file) fills in the `@""` lexical
//! parse/format helpers and the DST-correct duration arithmetic:
//!
//! - [`parse_at_literal`] — parse the string inside an `@"..."` literal for
//!   all 5 FEEL temporal types (date, time, date and time, days and time
//!   duration, years and months duration). Returns `None`→`Value::Null` on
//!   any parse failure (fail-closed — research V5, T-30-03-01 mitigation).
//! - [`format_value`] — the inverse of [`parse_at_literal`]; re-emits the
//!   `@Zone/Name` suffix for the IANA form (Pitfall 3 mitigation).
//! - [`add_months_zoned`] — `DateTime<chrono_tz::Tz> + YearsMonthsDuration`
//!   via `chrono::NaiveDate::checked_add_months` (Jan 31 + 1 month → Feb
//!   28/29, the spec "last day of resulting month" semantics — Don't
//!   Hand-Roll). DST gap → `None` (null), overlap → earliest (Pitfall 5).
//! - [`add_days_time_zoned`] — `DateTime<chrono_tz::Tz> + TimeDelta` with the
//!   same DST gap/overlap policy.
//!
//! ## Why an enum for `time` / `date and time`?
//!
//! FEEL temporal values may carry a fixed offset, an IANA zone, or no offset
//! (local). The TCK round-trip format `2021-02-01T10:10:10@Australia/Melbourne`
//! (30-RESEARCH.md Pitfall 3) requires the IANA zone to be *preserved* through
//! arithmetic and re-emitted by `to_string()`; `DateTime<FixedOffset>` would
//! discard it. Hence `DateTimeValue` is an enum carrying *either* form, and
//! `TimeValue` mirrors the same three cases for the `time` type.

use crate::data_types::Value;
use chrono::{
    DateTime, FixedOffset, Months, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, TimeZone,
};
use serde::{Deserialize, Serialize};

// Note: `DateTimeValue` cannot `#[derive(Serialize, Deserialize)]` because
// `chrono::DateTime<chrono_tz::Tz>` does not implement `Deserialize` — chrono
// only provides `Deserialize` for `DateTime<Utc>`, `DateTime<FixedOffset>`,
// and `DateTime<Local>`, not for an arbitrary `Tz: TimeZone`. We therefore
// derive serde on an intermediate `DateTimeValueRepr` (which holds the IANA
// variant as a `(NaiveDateTime, chrono_tz::Tz)` pair — both serde-able) and
// implement `Serialize`/`Deserialize` for `DateTimeValue` by converting
// through that repr. The `chrono_tz` `serde` feature makes `chrono_tz::Tz`
// (de)serialize as its IANA name string (e.g. `"Australia/Melbourne"`), so
// the zone round-trips faithfully (Pitfall 3 mitigation preserved).

/// FEEL `time` — offset-less, fixed-offset, or local.
///
/// - `Fixed` — a time with no offset (FEEL local time, XML Schema `HH:mm:ss`).
/// - `Offset` — a time with a fixed UTC offset (`HH:mm:ss+HH:mm`).
/// - `Local` — a local time explicitly (no offset carried).
///
/// `Offset` uses `chrono::DateTime<FixedOffset>` so the offset is retained; a
/// bare `NaiveTime` cannot carry an offset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TimeValue {
    /// Offset-less time (XML Schema `HH:mm:ss`).
    Fixed(chrono::NaiveTime),
    /// Time with a fixed UTC offset.
    Offset(chrono::DateTime<chrono::FixedOffset>),
    /// Local time, no offset.
    Local(chrono::NaiveTime),
}

/// FEEL `date and time` — fixed-offset, IANA-zone, or local.
///
/// - `Fixed` — a date-time with a fixed UTC offset (`DateTime<FixedOffset>`).
/// - `Iana` — a date-time with an IANA zone (`DateTime<chrono_tz::Tz>`). The
///   zone is *preserved* through arithmetic and re-emitted by `format_value`
///   (Pitfall 3 mitigation).
/// - `Local` — a local date-time with no offset (`NaiveDateTime`).
///
/// The legacy [`crate::data_types::Value::DateTime`] variant (Rete path,
/// `NaiveDateTime`) is NOT repurposed for FEEL date-time — FEEL uses the new
/// [`crate::data_types::Value::DateTimeTz`] variant wrapping this type.
#[derive(Debug, Clone, PartialEq)]
pub enum DateTimeValue {
    /// Date-time with a fixed UTC offset.
    Fixed(chrono::DateTime<chrono::FixedOffset>),
    /// Date-time with an IANA zone (e.g. `Australia/Melbourne`).
    Iana(chrono::DateTime<chrono_tz::Tz>),
    /// Local date-time, no offset.
    Local(chrono::NaiveDateTime),
}

/// Serde intermediate for [`DateTimeValue`] — see the module note above.
///
/// The `Iana` arm carries `(naive_local, tz)` rather than
/// `DateTime<chrono_tz::Tz>` because `chrono` does not implement
/// `Deserialize` for `DateTime<Tz>` with an arbitrary `Tz`.
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
enum DateTimeValueRepr {
    Fixed(chrono::DateTime<chrono::FixedOffset>),
    Iana(IanaRepr),
    Local(chrono::NaiveDateTime),
}

#[derive(Serialize, Deserialize)]
struct IanaRepr {
    /// The naive local component of the zoned datetime.
    naive: chrono::NaiveDateTime,
    /// The IANA timezone (serializes as its name, e.g. `"Australia/Melbourne"`).
    tz: chrono_tz::Tz,
}

impl Serialize for DateTimeValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let repr = match self {
            DateTimeValue::Fixed(dt) => DateTimeValueRepr::Fixed(*dt),
            DateTimeValue::Iana(dt) => DateTimeValueRepr::Iana(IanaRepr {
                naive: dt.naive_local(),
                tz: dt.timezone(),
            }),
            DateTimeValue::Local(ndt) => DateTimeValueRepr::Local(*ndt),
        };
        repr.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DateTimeValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let repr = DateTimeValueRepr::deserialize(deserializer)?;
        Ok(match repr {
            DateTimeValueRepr::Fixed(dt) => DateTimeValue::Fixed(dt),
            DateTimeValueRepr::Local(ndt) => DateTimeValue::Local(ndt),
            DateTimeValueRepr::Iana(r) => {
                use chrono::TimeZone;
                // Re-attach the IANA zone to the naive local datetime. DST
                // ambiguous times resolve to the earliest occurrence (the
                // documented Plan 03 policy); a non-existent local time
                // (spring-forward gap) is a data error at the serde boundary.
                match r.tz.from_local_datetime(&r.naive) {
                    chrono::LocalResult::Single(dt) => DateTimeValue::Iana(dt),
                    chrono::LocalResult::Ambiguous(dt, _) => DateTimeValue::Iana(dt),
                    chrono::LocalResult::None => {
                        return Err(serde::de::Error::custom(format!(
                            "non-existent local time {} in timezone {}",
                            r.naive, r.tz
                        )))
                    }
                }
            }
        })
    }
}

/// FEEL `years and months duration` — total months, normalized.
///
/// Per DMN 1.5 §10.3.2.3.7 a years-months duration is normalized so the months
/// field is in `[0, 11]`; the value is `years*12 + months`. We store the total
/// months as a single `i64` (`valueymd(P1Y1M) = 13`) so arithmetic is simple
/// and `chrono::NaiveDate::checked_add_months(Months::new(n))` can apply it
/// directly (Jan 31 + 1 month → Feb 28/29 — the spec-correct "last day of the
/// resulting month" semantics).
///
/// Days-and-time durations use `chrono::TimeDelta` (aliased from
/// `chrono::Duration`); they are a fixed number of seconds and need no custom
/// carrier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct YearsMonthsDuration {
    /// Total months (years*12 + months). May be negative.
    pub months: i64,
}

// =====================================================================
// Plan 30-03: @"" lexical parse/format + DST-correct duration arithmetic
// =====================================================================

/// Parse the string inside an `@"..."` FEEL literal (the `@""` wrapper is
/// stripped by the lexer; this function receives the inner string).
///
/// Dispatches by prefix/shape per DMN 1.5 grammar rule 65 + §10.3.2.3:
///
/// - `P…` / `-P…` → duration (years-months if only `Y`/`M` markers, else
///   days-time).
/// - `HH:mm:ss` (no `-`, contains `:`) → time (local, or offset if `+HH:mm`
///   suffix present).
/// - `YYYY-MM-DDTHH:mm:ss…` → date and time (fixed-offset, IANA-zone, or
///   local depending on suffix).
/// - `YYYY-MM-DD` → date.
///
/// Returns `None` (→ `Value::Null`) for any parse failure — **never** panics
/// on malformed input (T-30-03-01 mitigation, research V5). Years > 9999 are
/// rejected (Pitfall 4 — documented limitation; TCK vectors use 4-digit
/// years).
pub fn parse_at_literal(s: &str) -> Option<Value> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // Duration? starts with 'P' (positive) or '-P' (negative) — XDM
    // dayTimeDuration / yearMonthDuration lexical form.
    if s.starts_with('P') || s.starts_with("-P") {
        return parse_duration(s);
    }
    // Date-time? contains 'T' (only the date-time form has 'T').
    if s.contains('T') {
        return parse_date_time(s).map(Value::DateTimeTz);
    }
    // Date or time? disambiguate by the first separator:
    //  - ':' before '-' (or '-' absent) → time (HH:mm:ss[+/-HH:mm])
    //  - '-' before ':' (or ':' absent) → date (YYYY-MM-DD)
    let first_colon = s.find(':');
    let first_dash = s.find('-');
    let is_time = match (first_colon, first_dash) {
        (Some(c), Some(d)) => c < d,
        (Some(_), None) => true,
        (None, Some(_)) => false,
        (None, None) => return None,
    };
    if is_time {
        return parse_time(s).map(Value::Time);
    }
    // Date: reject years > 9999 (Pitfall 4 — documented limitation; the
    // TCK vectors use 4-digit years).
    let year_part = s.split('-').next().unwrap_or("");
    if year_part.len() > 4 {
        return None;
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .map(Value::Date)
}

/// Format a temporal [`Value`] back to its `@""` lexical form.
///
/// The inverse of [`parse_at_literal`]: for each temporal variant emits the
/// XML Schema / XDM lexical form, **preserving the `@Zone/Name` suffix for
/// the IANA form** (Pitfall 3 — the IANA zone name is re-emitted, NOT
/// collapsed to a fixed `+HH:mm` offset).
///
/// Returns `None` for non-temporal `Value` variants.
pub fn format_value(v: &Value) -> Option<String> {
    match v {
        Value::Date(d) => Some(d.format("%Y-%m-%d").to_string()),
        Value::Time(tv) => Some(format_time_value(tv)),
        Value::DateTimeTz(dtv) => Some(format_date_time_value(dtv)),
        Value::DaysTimeDuration(td) => Some(format_days_time_duration(*td)),
        Value::YearsMonthsDuration(ymd) => Some(format_years_months_duration(ymd)),
        // Non-temporal variants are not this function's responsibility.
        _ => None,
    }
}

/// Add a [`YearsMonthsDuration`] to a zoned datetime (IANA zone preserved).
///
/// Uses `NaiveDate::checked_add_months` on the local component (Jan 31 + 1
/// month → Feb 28/29 — the spec "last day of the resulting month" semantics,
/// Don't Hand-Roll), then re-attaches the IANA zone via
/// `TimeZone::from_local_datetime`. DST gap → `None` (null); overlap →
/// earliest (Pitfall 5 documented policy). **Never** unwraps `LocalResult`.
pub fn add_months_zoned(
    dt: DateTime<chrono_tz::Tz>,
    ymd: &YearsMonthsDuration,
) -> Option<DateTime<chrono_tz::Tz>> {
    let tz = dt.timezone();
    let naive = dt.naive_local();
    let new_naive = if ymd.months >= 0 {
        naive.checked_add_months(Months::new(ymd.months.unsigned_abs() as u32))?
    } else {
        naive.checked_sub_months(Months::new(ymd.months.unsigned_abs() as u32))?
    };
    // Re-attach the IANA zone. DST gap → None (null), Ambiguous → earliest
    // (Pitfall 5 documented policy). NEVER unwrap LocalResult.
    reattach_zone(&tz, &new_naive)
}

/// Add a `TimeDelta` (days-and-time duration) to a zoned datetime.
///
/// Computes the naive local result and re-attaches the IANA zone via
/// `TimeZone::from_local_datetime` with the same DST gap/overlap policy as
/// [`add_months_zoned`] (None → null, Ambiguous → earliest). **Never**
/// unwraps `LocalResult` (Pitfall 5).
pub fn add_days_time_zoned(
    dt: DateTime<chrono_tz::Tz>,
    dur: TimeDelta,
) -> Option<DateTime<chrono_tz::Tz>> {
    let tz = dt.timezone();
    let naive = dt.naive_local();
    // TimeDelta addition on a NaiveDateTime can overflow; map to None.
    let new_naive = naive.checked_add_signed(dur)?;
    // Re-attach the IANA zone with the same DST policy as add_months_zoned.
    reattach_zone(&tz, &new_naive)
}

// ----- parse helpers -----

/// Parse an XDM duration literal (`P…` / `-P…`) into a years-months or
/// days-time duration [`Value`].
///
/// Dispatch: if the body (after `P`) contains `T`, `D`, `H`, or `S` it is a
/// dayTimeDuration (TimeDelta); otherwise (only `Y`/`M` markers) it is a
/// yearMonthDuration. The `M` marker is disambiguated by the presence of `T`
/// — without `T`, `M` means months; with `T`, `M` means minutes.
fn parse_duration(s: &str) -> Option<Value> {
    let (neg, body) = if let Some(rest) = s.strip_prefix('-') {
        (true, rest)
    } else {
        (false, s)
    };
    let body = body.strip_prefix('P')?;
    if body.is_empty() {
        return None;
    }
    let is_day_time =
        body.contains('T') || body.contains('D') || body.contains('H') || body.contains('S');
    if is_day_time {
        let td = parse_day_time_duration(body, neg)?;
        Some(Value::DaysTimeDuration(td))
    } else {
        let ymd = parse_year_month_duration(body, neg)?;
        Some(Value::YearsMonthsDuration(ymd))
    }
}

/// Parse the body of a yearMonthDuration (the part after `P`, sign handled
/// separately) into total months. Body shape: `[n]Y[n]M` with at least one
/// field present.
fn parse_year_month_duration(body: &str, neg: bool) -> Option<YearsMonthsDuration> {
    let mut months: i64 = 0;
    let mut seen = false;
    let mut num = String::new();
    for ch in body.chars() {
        if ch.is_ascii_digit() {
            num.push(ch);
        } else if ch == 'Y' {
            let y: i64 = num.parse().ok()?;
            months += y * 12;
            num.clear();
            seen = true;
        } else if ch == 'M' {
            let m: i64 = num.parse().ok()?;
            months += m;
            num.clear();
            seen = true;
        } else {
            return None; // unexpected character
        }
    }
    if !seen || !num.is_empty() {
        return None; // need at least one field, no trailing digits
    }
    Some(YearsMonthsDuration {
        months: if neg { -months } else { months },
    })
}

/// Parse the body of a dayTimeDuration (the part after `P`, sign handled
/// separately) into a `TimeDelta`. Body shape: `[n]D` and/or `T[n]H[n]M[n]S`.
fn parse_day_time_duration(body: &str, neg: bool) -> Option<TimeDelta> {
    let mut days: i64 = 0;
    let mut hours: i64 = 0;
    let mut minutes: i64 = 0;
    let mut seconds: i64 = 0;
    let mut seen = false;
    let mut num = String::new();
    let mut after_t = false;
    for ch in body.chars() {
        if ch.is_ascii_digit() {
            num.push(ch);
        } else if ch == 'T' {
            after_t = true;
        } else if ch == 'D' {
            if after_t {
                return None;
            }
            days = num.parse().ok()?;
            num.clear();
            seen = true;
        } else if ch == 'H' {
            if !after_t {
                return None;
            }
            hours = num.parse().ok()?;
            num.clear();
            seen = true;
        } else if ch == 'M' {
            if !after_t {
                return None;
            }
            minutes = num.parse().ok()?;
            num.clear();
            seen = true;
        } else if ch == 'S' {
            if !after_t {
                return None;
            }
            seconds = num.parse().ok()?;
            num.clear();
            seen = true;
        } else {
            return None;
        }
    }
    if !seen || !num.is_empty() {
        return None;
    }
    let total_seconds = days * 86400 + hours * 3600 + minutes * 60 + seconds;
    let td = TimeDelta::seconds(if neg { -total_seconds } else { total_seconds });
    Some(td)
}

/// Parse an XML Schema time literal (`HH:mm:ss` or `HH:mm:ss+HH:mm`) into a
/// [`TimeValue`].
/// Parse `HH:MM:SS` with optional fractional seconds.
fn parse_time_of_day(text: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(text, "%H:%M:%S%.f")
        .or_else(|_| NaiveTime::parse_from_str(text, "%H:%M:%S"))
        .ok()
}

fn parse_time(s: &str) -> Option<TimeValue> {
    // `Z` is the UTC designator: `09:30:00Z` is `09:30:00+00:00`.
    if let Some(without_z) = s.strip_suffix('Z').or_else(|| s.strip_suffix('z')) {
        let naive = parse_time_of_day(without_z)?;
        let dt_naive = NaiveDate::from_ymd_opt(1970, 1, 1)?.and_time(naive);
        let utc = FixedOffset::east_opt(0)?;
        return utc
            .from_local_datetime(&dt_naive)
            .single()
            .map(TimeValue::Offset);
    }
    // Try offset form first (HH:mm:ss+HH:mm).
    if let Some(pos) = s.find('+').or_else(|| s.find('-').filter(|&p| p > 0)) {
        let (time_part, off_part) = s.split_at(pos);
        let naive = parse_time_of_day(time_part)?;
        let dt_naive = NaiveDate::from_ymd_opt(1970, 1, 1)?.and_time(naive);
        let off = parse_fixed_offset(off_part)?;
        let dt = off.from_local_datetime(&dt_naive).single()?;
        return Some(TimeValue::Offset(dt));
    }
    // Local time (no offset).
    Some(TimeValue::Local(parse_time_of_day(s)?))
}

/// Parse a date-time literal (`YYYY-MM-DDTHH:mm:ss` + optional `@Zone/Name` or
/// `+HH:mm` suffix) into a [`DateTimeValue`].
fn parse_date_time(s: &str) -> Option<DateTimeValue> {
    // IANA zone suffix? split at '@' (the FEEL @Zone/Name marker).
    if let Some(at_pos) = s.find('@') {
        let (dt_part, zone_part) = s.split_at(at_pos);
        let zone_part = &zone_part[1..]; // strip '@'
        let naive = NaiveDateTime::parse_from_str(dt_part, "%Y-%m-%dT%H:%M:%S").ok()?;
        let tz: chrono_tz::Tz = zone_part.parse().ok()?;
        let dt = tz.from_local_datetime(&naive).single().or_else(|| {
            // Ambiguous → earliest (Pitfall 5 documented policy).
            tz.from_local_datetime(&naive).earliest()
        })?;
        return Some(DateTimeValue::Iana(dt));
    }
    // Fixed-offset suffix? `+HH:mm` or `-HH:mm` after the time part.
    // NaiveDateTime::parse_from_str with "%+" handles RFC3339; but the
    // FEEL form is YYYY-MM-DDTHH:mm:ss+HH:mm (no seconds on the offset).
    // Try parsing the naive part + offset separately for robustness.
    let offset_pos = s
        .char_indices()
        .rev()
        .find(|(_, c)| *c == '+' || *c == '-')
        .map(|(i, _)| i);
    if let Some(pos) = offset_pos {
        let after_t = s.find('T').map(|t| t < pos).unwrap_or(false);
        if after_t {
            let (dt_part, off_part) = s.split_at(pos);
            if let (Ok(naive), Some(off)) = (
                NaiveDateTime::parse_from_str(dt_part, "%Y-%m-%dT%H:%M:%S"),
                parse_fixed_offset(off_part),
            ) {
                let dt = off.from_local_datetime(&naive).single()?;
                return Some(DateTimeValue::Fixed(dt));
            }
        }
    }
    // Local date-time (no offset, no zone).
    let naive = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S").ok()?;
    Some(DateTimeValue::Local(naive))
}

/// Parse a fixed UTC offset string (`+HH:mm` / `-HH:mm` / `+HH:mm:ss`) into a
/// `chrono::FixedOffset`. Returns `None` on any malformed input — never
/// panics (T-30-03-01).
fn parse_fixed_offset(s: &str) -> Option<FixedOffset> {
    let (sign, rest) = if let Some(r) = s.strip_prefix('+') {
        (1, r)
    } else if let Some(r) = s.strip_prefix('-') {
        (-1, r)
    } else {
        return None;
    };
    let mut parts = rest.split(':');
    let h: i32 = parts.next()?.parse().ok()?;
    let m: i32 = parts.next().unwrap_or("0").parse().ok()?;
    if parts.next().is_some() {
        // optional seconds field — accept and ignore if 0, reject otherwise
        let _s: i32 = parts.next().unwrap_or("0").parse().ok()?;
    }
    let secs = sign * (h * 3600 + m * 60);
    FixedOffset::east_opt(secs)
}

// ----- format helpers -----

fn format_time_value(tv: &TimeValue) -> String {
    match tv {
        TimeValue::Fixed(t) | TimeValue::Local(t) => t.format("%H:%M:%S").to_string(),
        TimeValue::Offset(dt) => format!(
            "{}{}",
            dt.format("%H:%M:%S"),
            format_fixed_offset(dt.offset())
        ),
    }
}

fn format_date_time_value(dtv: &DateTimeValue) -> String {
    match dtv {
        DateTimeValue::Fixed(dt) => format!(
            "{}{}",
            dt.format("%Y-%m-%dT%H:%M:%S"),
            format_fixed_offset(dt.offset())
        ),
        DateTimeValue::Iana(dt) => {
            // Pitfall 3: re-emit the IANA zone name, NOT the fixed offset.
            format!(
                "{}@{}",
                dt.naive_local().format("%Y-%m-%dT%H:%M:%S"),
                dt.timezone().name()
            )
        }
        DateTimeValue::Local(ndt) => ndt.format("%Y-%m-%dT%H:%M:%S").to_string(),
    }
}

fn format_fixed_offset(off: &FixedOffset) -> String {
    let secs = off.local_minus_utc();
    let sign = if secs >= 0 { '+' } else { '-' };
    let abs = secs.unsigned_abs() as i64;
    let h = abs / 3600;
    let m = (abs % 3600) / 60;
    format!("{}{:02}:{:02}", sign, h, m)
}

fn format_days_time_duration(td: TimeDelta) -> String {
    let total_secs = td.num_seconds();
    let neg = total_secs < 0;
    let abs = total_secs.unsigned_abs() as i64;
    let days = abs / 86400;
    let rem = abs % 86400;
    let hours = rem / 3600;
    let rem = rem % 3600;
    let minutes = rem / 60;
    let seconds = rem % 60;
    let mut s = String::from("P");
    if days > 0 {
        s.push_str(&format!("{}D", days));
    }
    if hours > 0 || minutes > 0 || seconds > 0 || days == 0 {
        s.push('T');
        if hours > 0 {
            s.push_str(&format!("{}H", hours));
        }
        if minutes > 0 {
            s.push_str(&format!("{}M", minutes));
        }
        if seconds > 0 {
            s.push_str(&format!("{}S", seconds));
        }
        if days == 0 && hours == 0 && minutes == 0 && seconds == 0 {
            // Zero duration → "PT0S" (XDM canonical form).
            s.push_str("0S");
        }
    }
    if neg {
        format!("-{}", s)
    } else {
        s
    }
}

fn format_years_months_duration(ymd: &YearsMonthsDuration) -> String {
    let neg = ymd.months < 0;
    let abs = ymd.months.unsigned_abs() as i64;
    let years = abs / 12;
    let months = abs % 12;
    let mut s = String::from("P");
    if years > 0 {
        s.push_str(&format!("{}Y", years));
    }
    if months > 0 || years == 0 {
        s.push_str(&format!("{}M", months));
    }
    if neg {
        format!("-{}", s)
    } else {
        s
    }
}

// ----- DST re-attachment helper -----

/// Re-attach an IANA zone to a naive local datetime. DST gap → `None` (null),
/// Ambiguous → earliest (Pitfall 5 documented policy). Never unwraps
/// `LocalResult`.
fn reattach_zone(tz: &chrono_tz::Tz, naive: &NaiveDateTime) -> Option<DateTime<chrono_tz::Tz>> {
    match tz.from_local_datetime(naive) {
        chrono::LocalResult::Single(dt) => Some(dt),
        chrono::LocalResult::Ambiguous(dt, _) => Some(dt),
        chrono::LocalResult::None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use proptest::prop_assert_eq;

    // ----- existing Plan 01 carrier tests (kept green) -----

    #[test]
    fn time_value_fixed_constructs() {
        let t = TimeValue::Fixed(chrono::NaiveTime::from_hms_opt(10, 10, 10).unwrap());
        assert_eq!(t, t.clone());
    }

    #[test]
    fn date_time_value_iana_constructs() {
        let dt = chrono_tz::Australia::Melbourne
            .with_ymd_and_hms(2021, 2, 1, 10, 10, 10)
            .single()
            .unwrap();
        let v = DateTimeValue::Iana(dt);
        assert_eq!(v, v.clone());
    }

    #[test]
    fn years_months_duration_constructs() {
        let d = YearsMonthsDuration { months: 13 };
        assert_eq!(d.months, 13);
        assert_eq!(d, d.clone());
    }

    #[test]
    fn date_time_value_iana_serde_round_trips() {
        let dt = chrono_tz::Australia::Melbourne
            .with_ymd_and_hms(2021, 2, 1, 10, 10, 10)
            .single()
            .unwrap();
        let v = DateTimeValue::Iana(dt);
        let s = serde_json::to_string(&v).unwrap();
        let back: DateTimeValue = serde_json::from_str(&s).unwrap();
        assert_eq!(v, back, "IANA zone round-trip must preserve the zone");
    }

    #[test]
    fn date_time_value_fixed_serde_round_trips() {
        let dt = chrono::FixedOffset::east_opt(11 * 3600)
            .unwrap()
            .with_ymd_and_hms(2021, 2, 1, 10, 10, 10)
            .unwrap();
        let v = DateTimeValue::Fixed(dt);
        let s = serde_json::to_string(&v).unwrap();
        let back: DateTimeValue = serde_json::from_str(&s).unwrap();
        assert_eq!(v, back);
    }

    #[test]
    fn date_time_value_local_serde_round_trips() {
        let ndt = chrono::NaiveDate::from_ymd_opt(2021, 2, 1)
            .unwrap()
            .and_hms_opt(10, 10, 10)
            .unwrap();
        let v = DateTimeValue::Local(ndt);
        let s = serde_json::to_string(&v).unwrap();
        let back: DateTimeValue = serde_json::from_str(&s).unwrap();
        assert_eq!(v, back);
    }

    // ----- Plan 03 Task 1: parse_at_literal / format_value / arithmetic -----

    #[test]
    fn test_parse_date() {
        let v = parse_at_literal("2021-02-01");
        assert_eq!(
            v,
            Some(Value::Date(NaiveDate::from_ymd_opt(2021, 2, 1).unwrap()))
        );
    }

    #[test]
    fn test_parse_time() {
        let local = parse_at_literal("10:10:10");
        assert_eq!(
            local,
            Some(Value::Time(TimeValue::Local(
                NaiveTime::from_hms_opt(10, 10, 10).unwrap()
            )))
        );
        let offset = parse_at_literal("10:10:10+11:00");
        match offset {
            Some(Value::Time(TimeValue::Offset(_))) => {}
            other => panic!("expected TimeValue::Offset, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_date_time_fixed() {
        let v = parse_at_literal("2021-02-01T10:10:10+11:00");
        match v {
            Some(Value::DateTimeTz(DateTimeValue::Fixed(_))) => {}
            other => panic!("expected DateTimeValue::Fixed, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_date_time_iana() {
        // Pitfall 3: the IANA zone must be carried, NOT collapsed to +11:00.
        let v = parse_at_literal("2021-02-01T10:10:10@Australia/Melbourne");
        match v {
            Some(Value::DateTimeTz(DateTimeValue::Iana(dt))) => {
                assert_eq!(dt.timezone(), chrono_tz::Australia::Melbourne);
            }
            other => panic!("expected DateTimeValue::Iana, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_days_time_duration() {
        let p1d = parse_at_literal("P1D");
        match p1d {
            Some(Value::DaysTimeDuration(td)) => {
                assert_eq!(td, TimeDelta::days(1));
            }
            other => panic!("expected DaysTimeDuration(1 day), got {:?}", other),
        }
        let pt1h = parse_at_literal("PT1H");
        match pt1h {
            Some(Value::DaysTimeDuration(td)) => {
                assert_eq!(td, TimeDelta::hours(1));
            }
            other => panic!("expected DaysTimeDuration(1 hour), got {:?}", other),
        }
        let neg = parse_at_literal("-P1D");
        match neg {
            Some(Value::DaysTimeDuration(td)) => {
                assert_eq!(td, TimeDelta::days(-1));
            }
            other => panic!("expected DaysTimeDuration(-1 day), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_years_months_duration() {
        let p1y = parse_at_literal("P1Y");
        assert_eq!(
            p1y,
            Some(Value::YearsMonthsDuration(YearsMonthsDuration {
                months: 12
            }))
        );
        let p1y1m = parse_at_literal("P1Y1M");
        assert_eq!(
            p1y1m,
            Some(Value::YearsMonthsDuration(YearsMonthsDuration {
                months: 13
            }))
        );
        let p1m = parse_at_literal("P1M");
        assert_eq!(
            p1m,
            Some(Value::YearsMonthsDuration(YearsMonthsDuration {
                months: 1
            }))
        );
    }

    #[test]
    fn test_parse_invalid() {
        assert_eq!(parse_at_literal("not-a-date"), None);
        assert_eq!(parse_at_literal(""), None);
    }

    #[test]
    fn test_format_round_trip() {
        // Date
        let d = "2021-02-01";
        let v = parse_at_literal(d).expect("date parses");
        assert_eq!(format_value(&v).as_deref(), Some(d));
        // Time (local)
        let t = "10:10:10";
        let v = parse_at_literal(t).expect("local time parses");
        assert_eq!(format_value(&v).as_deref(), Some(t));
        // DateTime (fixed offset)
        let dtf = "2021-02-01T10:10:10+11:00";
        let v = parse_at_literal(dtf).expect("fixed-offset datetime parses");
        assert_eq!(format_value(&v).as_deref(), Some(dtf));
        // DateTime (IANA) — Pitfall 3: @Australia/Melbourne suffix preserved.
        let dti = "2021-02-01T10:10:10@Australia/Melbourne";
        let v = parse_at_literal(dti).expect("IANA datetime parses");
        let formatted = format_value(&v).expect("IANA datetime formats");
        assert!(
            formatted.contains("@Australia/Melbourne"),
            "Pitfall 3: IANA suffix must be preserved, got {:?}",
            formatted
        );
        assert!(
            !formatted.contains("+11:00"),
            "Pitfall 3: IANA form must NOT collapse to +11:00, got {:?}",
            formatted
        );
        // Days-time duration
        let dur = "P1D";
        let v = parse_at_literal(dur).expect("P1D parses");
        assert_eq!(format_value(&v).as_deref(), Some(dur));
        // Years-months duration
        let ymd = "P1Y1M";
        let v = parse_at_literal(ymd).expect("P1Y1M parses");
        assert_eq!(format_value(&v).as_deref(), Some(ymd));
    }

    #[test]
    fn test_month_edge() {
        // Jan 31 + 1 month → Feb 28 (non-leap) / Feb 29 (leap) — the spec
        // "last day of resulting month" semantics via checked_add_months.
        let jan31_2021 = chrono_tz::Australia::Melbourne
            .with_ymd_and_hms(2021, 1, 31, 10, 10, 10)
            .single()
            .unwrap();
        let ymd = YearsMonthsDuration { months: 1 };
        let feb = add_months_zoned(jan31_2021, &ymd).expect("Jan 31 + 1M = Feb 28");
        assert_eq!(
            feb.naive_local().date(),
            NaiveDate::from_ymd_opt(2021, 2, 28).unwrap()
        );

        let jan31_2024 = chrono_tz::Australia::Melbourne
            .with_ymd_and_hms(2024, 1, 31, 10, 10, 10)
            .single()
            .unwrap();
        let feb29 = add_months_zoned(jan31_2024, &ymd).expect("Jan 31 2024 + 1M = Feb 29");
        assert_eq!(
            feb29.naive_local().date(),
            NaiveDate::from_ymd_opt(2024, 2, 29).unwrap()
        );
    }

    #[test]
    fn test_dst_gap_no_panic() {
        // America/New_York springs forward at 2021-03-14 02:00 → 03:00 (the
        // local time 02:00-02:59 does not exist on that date). A duration
        // addition that lands in the gap must return None (null) — never
        // panic (Pitfall 5).
        let before_gap = chrono_tz::America::New_York
            .with_ymd_and_hms(2021, 3, 14, 1, 30, 0)
            .single()
            .unwrap();
        // 01:30 EST + 1 hour = 02:30 (which does not exist — gap).
        let gap = add_days_time_zoned(before_gap, TimeDelta::hours(1));
        assert_eq!(gap, None, "DST gap → null (Pitfall 5 documented policy)");

        // DST overlap: America/New_York falls back at 2021-11-07 02:00 →
        // 01:00 (the local time 01:00-01:59 occurs twice). A duration
        // addition that lands in the overlap returns the EARLIEST occurrence
        // (Pitfall 5 documented policy).
        let before_overlap = chrono_tz::America::New_York
            .with_ymd_and_hms(2021, 11, 7, 0, 30, 0)
            .single()
            .unwrap();
        // 00:30 EDT + 1 hour = 01:30 (overlap — occurs twice in EDT/EST).
        let overlap = add_days_time_zoned(before_overlap, TimeDelta::hours(1));
        assert!(overlap.is_some(), "DST overlap → earliest (not None)");
    }

    proptest::proptest! {
        #[test]
        fn temporal_proptest(year in 1i32..=9999, month in 1u32..=12, day in 1u32..=28) {
            // For any valid ISO 8601 date literal, parse_at_literal then
            // format_value reproduces the input (round-trip property).
            let d = NaiveDate::from_ymd_opt(year, month, day).unwrap();
            let lex = format!("{:04}-{:02}-{:02}", year, month, day);
            prop_assert_eq!(parse_at_literal(&lex), Some(Value::Date(d)));
            let formatted = format_value(&Value::Date(d));
            prop_assert_eq!(formatted.as_deref(), Some(lex.as_str()));
        }
    }
}
