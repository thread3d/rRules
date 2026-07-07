//! FEEL temporal carrier types — skeleton declarations.
//!
//! Plan 30-01 lands `TimeValue`, `DateTimeValue`, and `YearsMonthsDuration`.
//! The `@""` lexical parse/format helpers and DST-correct duration arithmetic
//! (`parse_at_literal`, `format_value`, `add_months_zoned`,
//! `add_days_time_zoned`) land in Plan 03.
//!
//! ## Why an enum for `time` / `date and time`?
//!
//! FEEL temporal values may carry a fixed offset, an IANA zone, or no offset
//! (local). The TCK round-trip format `2021-02-01T10:10:10@Australia/Melbourne`
//! (30-RESEARCH.md Pitfall 3) requires the IANA zone to be *preserved* through
//! arithmetic and re-emitted by `to_string()`; `DateTime<FixedOffset>` would
//! discard it. Hence `DateTimeValue` is an enum carrying *either* form, and
//! `TimeValue` mirrors the same three cases for the `time` type.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_value_fixed_constructs() {
        let t = TimeValue::Fixed(chrono::NaiveTime::from_hms_opt(10, 10, 10).unwrap());
        assert_eq!(t, t.clone());
    }

    #[test]
    fn date_time_value_iana_constructs() {
        // The IANA arm must carry a chrono_tz::Tz-backed DateTime (Pitfall 3).
        use chrono::TimeZone;
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
        // The IANA zone must survive a serde round-trip (Pitfall 3 mitigation).
        use chrono::TimeZone;
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
        use chrono::TimeZone;
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
}