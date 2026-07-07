use tanzim_validate::{Path, PathKind, Validator};
use tanzim_value::Value;

fn string(text: &str) -> Value {
    Value::String(text.to_string())
}

#[test]
fn absolute_and_relative() {
    assert!(
        Path::new()
            .absolute()
            .validate(&mut string("/etc/app"))
            .is_ok()
    );
    assert!(Path::new().absolute().validate(&mut string("app")).is_err());
    assert!(
        Path::new()
            .relative()
            .validate(&mut string("app/conf"))
            .is_ok()
    );
}

#[test]
fn extension_filter() {
    assert!(
        Path::new()
            .extension("toml")
            .validate(&mut string("a.toml"))
            .is_ok()
    );
    assert!(
        Path::new()
            .extension("toml")
            .validate(&mut string("a.json"))
            .is_err()
    );
}

#[test]
fn must_exist_uses_filesystem() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let mut here = string(manifest);
    assert!(
        Path::new()
            .must_exist()
            .kind(PathKind::Dir)
            .validate(&mut here)
            .is_ok()
    );
    let mut missing = string("/this/path/should/not/exist/xyzzy");
    assert!(Path::new().must_exist().validate(&mut missing).is_err());
}

#[test]
fn format_only_never_touches_fs() {
    let mut value = string("/nope/not/here.toml");
    assert!(
        Path::new()
            .absolute()
            .extension("toml")
            .validate(&mut value)
            .is_ok()
    );
}
