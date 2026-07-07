use tanzim_validate::{RegexPattern, Validator};
use tanzim_value::Value;

#[test]
fn accepts_valid_and_rejects_invalid() {
    assert!(
        RegexPattern::new()
            .validate(&mut Value::String("^a.*$".into()))
            .is_ok()
    );
    assert!(
        RegexPattern::new()
            .validate(&mut Value::String("(".into()))
            .is_err()
    );
}
