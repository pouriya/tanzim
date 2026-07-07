use tanzim_validate::{Percentage, Validator};
use tanzim_value::Value;

#[test]
fn int_and_ratio() {
    assert!(Percentage::new().validate(&mut Value::Int(50)).is_ok());
    assert!(Percentage::new().validate(&mut Value::Int(150)).is_err());
    assert!(Percentage::new().validate(&mut Value::Float(0.5)).is_ok());
    assert!(Percentage::new().validate(&mut Value::Float(1.5)).is_err());
}
