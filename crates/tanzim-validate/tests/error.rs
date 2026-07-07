use tanzim_validate::{Error, ErrorKind};
use tanzim_value::{Location, ValueType};

#[test]
fn nested_error_renders_path_and_innermost_location() {
    let leaf_loc = Location::at("file", "config.yaml", Some(3), Some(9), None);
    let outer_loc = Location::at("file", "config.yaml", Some(2), Some(1), None);
    let error = Error::new(ErrorKind::Type {
        expected: ValueType::Int,
        found: ValueType::String,
    })
    .under_key("port", &leaf_loc)
    .under_index(0, &outer_loc)
    .under_key("servers", &outer_loc);

    let message = error.to_string();
    assert!(message.starts_with("servers[0].port: expected integer, found string"));
    assert!(message.contains("config.yaml:3:9"));
}

#[test]
fn alternate_display_shows_caret_snippet() {
    let text = "name: app\nport: nope\n";
    let location = tanzim_value::Location::in_text(
        tanzim_source::Source::named("file").with_resource("config.yaml"),
        text,
        Some(2),
        Some(7),
        Some(4),
    );
    let error = Error::new(ErrorKind::Type {
        expected: ValueType::Int,
        found: ValueType::String,
    })
    .with_location(&location);

    let plain = error.to_string();
    assert!(!plain.contains('\n'));
    assert!(!plain.contains('^'));

    let alternate = format!("{error:#}");
    assert!(alternate.contains("port: nope"), "{alternate}");
    assert!(alternate.contains("^^^^"), "{alternate}");
}
