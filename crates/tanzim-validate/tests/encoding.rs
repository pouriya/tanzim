use tanzim_validate::{Base64, Hex, Validator};
use tanzim_value::Value;

#[test]
fn base64_roundtrip() {
    assert!(
        Base64::new()
            .validate(&mut Value::String("aGVsbG8=".into()))
            .is_ok()
    );
    assert!(
        Base64::new()
            .validate(&mut Value::String("not base64!".into()))
            .is_err()
    );
}

#[test]
fn hex_digits() {
    assert!(
        Hex::new()
            .validate(&mut Value::String("deadBEEF".into()))
            .is_ok()
    );
    assert!(
        Hex::new()
            .validate(&mut Value::String("xyz".into()))
            .is_err()
    );
    assert!(
        Hex::new()
            .validate(&mut Value::String("abc".into()))
            .is_err()
    );
}
