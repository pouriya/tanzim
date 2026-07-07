use tanzim_validate::{Date, DateTime, Validator};
use tanzim_value::Value;

#[test]
fn datetime_accepts_rfc3339() {
    assert!(
        DateTime::new()
            .validate(&mut Value::String("2024-01-02T15:04:05Z".into()))
            .is_ok()
    );
    assert!(
        DateTime::new()
            .validate(&mut Value::String("yesterday".into()))
            .is_err()
    );
}

#[test]
fn date_accepts_calendar_date() {
    assert!(
        Date::new()
            .validate(&mut Value::String("2024-01-02".into()))
            .is_ok()
    );
    assert!(
        Date::new()
            .validate(&mut Value::String("2024-13-99".into()))
            .is_err()
    );
}
