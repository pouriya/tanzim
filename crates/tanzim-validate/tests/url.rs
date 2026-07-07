use tanzim_validate::{Url, Validator};
use tanzim_value::Value;

fn string(text: &str) -> Value {
    Value::String(text.to_string())
}

#[test]
fn accepts_url() {
    assert!(
        Url::new()
            .validate(&mut string("https://example.com/x"))
            .is_ok()
    );
    assert!(Url::new().validate(&mut string("not a url")).is_err());
}

#[test]
fn restricts_scheme_and_host() {
    let validator = Url::new().schemes(["https"]).require_host();
    assert!(
        validator
            .validate(&mut string("https://example.com"))
            .is_ok()
    );
    assert!(
        validator
            .validate(&mut string("http://example.com"))
            .is_err()
    );
    assert!(validator.validate(&mut string("mailto:a@b.com")).is_err());
}
