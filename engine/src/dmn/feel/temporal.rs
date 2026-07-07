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
use chrono::{DateTime, FixedOffset, Months, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, TimeDelta};
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
    None // RED stub — Plan 03 Task 1 GREEN fills this in.
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
    None // RED stub — Plan 03 Task 1 GREEN fills this in.
}

/// Add a [`YearsMonthsDuration`] to a zoned datetime (IANA zone preserved).
///
/// Uses `NaiveDate::checked_add_months` on the local component (Jan 31 + 1
/// month → Feb 28/29 — the spec "last day of the resulting month" semantics,
/// Don't Hand-Roll), then re-attaches the IANA zone via
/// `TimeZone::from_local_datetime`. DST gap → `None` (null); overlap →
/// earliest (Pitfall 5 documented policy). **Never** unwraps `LocalResult`.
pub fn add_months_zoned(
    _dt: DateTime<chrono_tz::Tz>,
    _ymd: &YearsMonthsDuration,
) -> Option<DateTime<chrono_tz::Tz>> {
    None // RED stub — Plan 03 Task 1 GREEN fills this in.
}

/// Add a `TimeDelta` (days-and-time duration) to a zoned datetime.
///
/// Computes the naive local result and re-attaches the IANA zone via
/// `TimeZone::from_local_datetime` with the same DST gap/overlap policy as
/// [`add_months_zoned`] (None → null, Ambiguous → earliest). **Never**
/// unwraps `LocalResult` (Pitfall 5).
pub fn add_days_time_zoned(
    _dt: DateTime<chrono_tz::Tz>,
    _dur: TimeDelta,
) -> Option<DateTime<chrono_tz::Tz>> {
    None // RED stub — Plan 03 Task 1 GREEN fills this in.
}

// Silence unused-import warnings in the RED stub phase; the imports are used
// by the GREEN implementation that follows.
#[allow(dead_code)]
fn _red_imports_anchor() {
    let _ = (NaiveDate::default(), NaiveTime::default(), NaiveDateTime::default());
    let _ = Months::new(0);
    let _ = FixedOffset::east_opt(0);
    let _: DateTime<FixedOffset> = DateTime::<FixedOffset>::default();
    let _: Option<DateTime<chrono_tz::Tz>> = None;
    let _: TimeDelta = TimeDelta::zero();
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
        assert_eq!(feb.naive_local().date(), NaiveDate::from_ymd_opt(2021, 2, 28).unwrap());

        let jan31_2024 = chrono_tz::Australia::Melbourne
            .with_ymd_and_hms(2024, 1, 31, 10, 10, 10)
            .single()
            .unwrap();
        let feb29 = add_months_zoned(jan31_2024, &ymd).expect("Jan 31 2024 + 1M = Feb 29");
        assert_eq!(feb29.naive_local().date(), NaiveDate::from_ymd_opt(2024, 2, 29).unwrap());
    }

    #[test]
    fn test_dst_gap_no_panic() {
        // America/Sao_Paulo springs forward at 2021-10-17 00:00 → 01:00
        // (the local time 00:00-00:59 does not exist on that date). A
        // days-and-time-duration addition that lands in the gap must return
        // None (null) — never panic (Pitfall 5).
        //
        // Construct a datetime just before the gap and add a duration that
        // lands inside it. (Sao_Paulo's 2021 spring-forward: midnight-1am
        // skipped on 2021-10-17.)
        let before = chrono_tz::America::Sao_Paulo
            .with_ymd_and_hms(2021, 10, 16, 23, 30, 0)
            .single()
            .unwrap();
        // Add 30 minutes → lands at 00:00 on 2021-10-17 which does not exist.
        let gap = add_days_time_zoned(before, TimeDelta::minutes(30));
        assert_eq!(
            gap, None,
            "DST gap → null (Pitfall 5 documented policy)"
        );

        // DST overlap: Sao_Paulo 2021-02-20 23:00-23:59 occurs twice (fall
        // back). A datetime landing in the overlap returns the EARLIEST.
        // Construct a naive datetime in the overlap and add 0 to exercise
        // from_local_datetime's Ambiguous branch via add_days_time_zoned.
        let in_overlap = chrono_tz::America::Sao_Paulo
            .with_ymd_and_hms(2021, 2, 20, 23, 30, 0)
            .single()
            .unwrap();
        let zero = add_days_time_zoned(in_overlap, TimeDelta::zero());
        // Earliest is returned — single() would have returned None for an
        // ambiguous time, so Some(...) confirms the earliest policy.
        assert!(zero.is_some(), "DST overlap → earliest (not None)");
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