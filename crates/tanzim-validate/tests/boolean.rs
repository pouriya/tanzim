use tanzim_validate::{Bool, ErrorKind, Validator};
use tanzim_value::Value;

#[test]
fn accepts_bool() {
    let mut value = Value::Bool(true);
    assert!(Bool::new().validate(&mut value).is_ok());
}

#[test]
fn rejects_non_bool() {
    let mut value = Value::Int(1);
    let error = Bool::new().validate(&mut value).unwrap_err();
    assert!(matches!(error.kind, ErrorKind::Type { .. }));
}
