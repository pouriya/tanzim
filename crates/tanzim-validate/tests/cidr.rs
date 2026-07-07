use tanzim_validate::{Cidr, Validator};
use tanzim_value::Value;

#[test]
fn accepts_and_rejects() {
    assert!(
        Cidr::new()
            .validate(&mut Value::String("10.0.0.0/8".into()))
            .is_ok()
    );
    assert!(
        Cidr::new()
            .validate(&mut Value::String("10.0.0.0".into()))
            .is_err()
    );
}
