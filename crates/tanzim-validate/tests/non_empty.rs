use tanzim_validate::{NonEmpty, Validator};
use tanzim_value::Value;

#[test]
fn rejects_blank() {
    assert!(
        NonEmpty::new()
            .validate(&mut Value::String("x".into()))
            .is_ok()
    );
    assert!(
        NonEmpty::new()
            .validate(&mut Value::String("   ".into()))
            .is_err()
    );
}
