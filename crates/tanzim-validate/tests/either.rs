use tanzim_validate::{Bool, Either, ErrorKind, Integer, Validator};
use tanzim_value::Value;

#[test]
fn accepts_when_either_matches() {
    let validator = Either::new(Integer::new(), Bool::new());
    assert!(validator.validate(&mut Value::Int(3)).is_ok());
    assert!(validator.validate(&mut Value::Bool(true)).is_ok());
}

#[test]
fn commits_coercion_of_winning_branch() {
    let validator = Either::new(Integer::new(), Bool::new());
    let mut value = Value::String("5".into());
    validator.validate(&mut value).unwrap();
    assert_eq!(value, Value::Int(5));
}

#[test]
fn original_value_preserved_for_second_attempt() {
    let validator = Either::new(Integer::new(), Bool::new());
    let mut value = Value::Bool(true);
    validator.validate(&mut value).unwrap();
    assert_eq!(value, Value::Bool(true));
}

#[test]
fn combines_errors_when_both_fail() {
    let validator = Either::new(Bool::new(), Integer::new());
    let mut value = Value::String("nope".into());
    let error = validator.validate(&mut value).unwrap_err();
    assert!(matches!(error.kind, ErrorKind::Either { .. }));
    assert_eq!(value, Value::String("nope".into()));
}
