use tanzim_validate::{Number, Validator};
use tanzim_value::Value;

#[test]
fn accepts_int_and_float_without_converting() {
    let mut int_value = Value::Int(3);
    Number::new().validate(&mut int_value).unwrap();
    assert_eq!(int_value, Value::Int(3));

    let mut float_value = Value::Float(3.5);
    Number::new().validate(&mut float_value).unwrap();
    assert_eq!(float_value, Value::Float(3.5));
}

#[test]
fn rejects_non_numbers() {
    assert!(
        Number::new()
            .validate(&mut Value::String("3".into()))
            .is_err()
    );
}

#[test]
fn bounds_and_sign() {
    assert!(
        Number::new()
            .range(0.0, 10.0)
            .validate(&mut Value::Int(5))
            .is_ok()
    );
    assert!(
        Number::new()
            .range(0.0, 10.0)
            .validate(&mut Value::Float(11.0))
            .is_err()
    );
    assert!(
        Number::new()
            .positive()
            .validate(&mut Value::Float(0.0))
            .is_err()
    );
    assert!(
        Number::new()
            .non_negative()
            .validate(&mut Value::Int(0))
            .is_ok()
    );
}
