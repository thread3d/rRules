//! FEEL temporal carrier types — skeleton declarations.
//!
//! Plan 30-01 lands `TimeValue`, `DateTimeValue`, and `YearsMonthsDuration`.
//! The `@""` lexical parse/format helpers and DST-correct duration arithmetic
//! (`parse_at_literal`, `format_value`, `add_months_zoned`,
//! `add_days_time_zoned`) land in Plan 03.

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
}