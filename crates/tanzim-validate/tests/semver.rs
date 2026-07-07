use tanzim_validate::{Semver, Validator};
use tanzim_value::Value;

#[test]
fn accepts_and_rejects() {
    assert!(
        Semver::new()
            .validate(&mut Value::String("1.2.3".into()))
            .is_ok()
    );
    assert!(
        Semver::new()
            .validate(&mut Value::String("1.2".into()))
            .is_err()
    );
}
