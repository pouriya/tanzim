use tanzim_validate::{Uuid, Validator};
use tanzim_value::Value;

#[test]
fn accepts_and_rejects() {
    assert!(
        Uuid::new()
            .validate(&mut Value::String(
                "67e55044-10b1-426f-9247-bb680e5fe0c8".into()
            ))
            .is_ok()
    );
    assert!(
        Uuid::new()
            .validate(&mut Value::String("nope".into()))
            .is_err()
    );
}
