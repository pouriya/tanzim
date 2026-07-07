use tanzim_validate::{Enum, ErrorKind, Validator};
use tanzim_value::Value;

#[test]
fn string_membership() {
    let validator = Enum::new([Value::String("debug".into()), Value::String("info".into())]);
    assert!(
        validator
            .validate(&mut Value::String("info".into()))
            .is_ok()
    );
    let error = validator
        .validate(&mut Value::String("trace".into()))
        .unwrap_err();
    assert!(matches!(error.kind, ErrorKind::NotAllowed { .. }));
}

#[test]
fn accepts_non_string_types() {
    let validator = Enum::new([Value::Int(1), Value::Int(2), Value::Bool(true)]);
    assert!(validator.validate(&mut Value::Int(2)).is_ok());
    assert!(validator.validate(&mut Value::Bool(true)).is_ok());
    assert!(validator.validate(&mut Value::Int(3)).is_err());
}

#[test]
fn case_insensitive_strings() {
    let validator = Enum::new([Value::String("Info".into())]).case_insensitive();
    assert!(
        validator
            .validate(&mut Value::String("INFO".into()))
            .is_ok()
    );
}
