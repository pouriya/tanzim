use tanzim_validate::{ErrorKind, Str, Validator};
use tanzim_value::Value;

#[test]
fn accepts_string() {
    let mut value = Value::String("hi".into());
    assert!(Str::new().validate(&mut value).is_ok());
}

#[test]
fn rejects_non_string() {
    let mut value = Value::Int(1);
    let error = Str::new().validate(&mut value).unwrap_err();
    assert!(matches!(error.kind, ErrorKind::Type { .. }));
}

#[test]
fn enforces_min_chars() {
    let mut value = Value::String("".into());
    let error = Str::new().min_chars(1).validate(&mut value).unwrap_err();
    assert!(matches!(error.kind, ErrorKind::TooShort { .. }));
}

#[test]
fn enforces_max_chars() {
    let mut value = Value::String("toolong".into());
    let error = Str::new().max_chars(3).validate(&mut value).unwrap_err();
    assert!(matches!(error.kind, ErrorKind::TooLong { .. }));
}

#[cfg(feature = "regex")]
#[test]
fn regex_matches_and_rejects() {
    let validator = Str::new().regex("^[a-z]+$").unwrap();
    let mut ok = Value::String("abc".into());
    assert!(validator.validate(&mut ok).is_ok());
    let mut bad = Value::String("abc1".into());
    let error = validator.validate(&mut bad).unwrap_err();
    assert!(matches!(error.kind, ErrorKind::PatternMismatch { .. }));
}
