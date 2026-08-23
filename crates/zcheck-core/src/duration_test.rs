//! Tests for bounded manifest duration syntax.

use super::DurationSpec;

#[test]
fn parses_supported_units_without_normalizing_the_source() {
    let duration = "20m".parse::<DurationSpec>();
    assert_eq!(
        duration.as_ref().map(DurationSpec::milliseconds),
        Ok(1_200_000)
    );
    assert_eq!(duration.as_ref().map(DurationSpec::as_str), Ok("20m"));
}

#[test]
fn rejects_zero_missing_units_and_overflow() {
    for source in ["0s", "20", "-1s", "18446744073709551615h"] {
        assert!(source.parse::<DurationSpec>().is_err(), "accepted {source}");
    }
}
