use tanzim_source::Source;
use tanzim_value::{Error, Location};

fn source() -> Source {
    Source::named("file").with_resource("config.toml")
}

#[test]
fn default_display_is_single_line() {
    let error = Error::UnsupportedType {
        location: Box::new(Location::at("file", "config.toml", Some(2), Some(7), None)),
        found: "datetime",
    };
    let message = error.to_string();
    assert!(!message.contains('\n'));
    assert!(!message.contains('^'));
    assert!(message.contains("file:config.toml:2:7"));
}

#[test]
fn alternate_display_underlines_token() {
    let text = "foo: bar\nbaz: datetime\n";
    let error = Error::UnsupportedType {
        location: Box::new(Location::in_text(source(), text, Some(2), Some(6), Some(8))),
        found: "datetime",
    };
    let message = format!("{error:#}");
    assert!(message.contains("^^^^"));
    assert!(message.contains("baz: datetime"));
}

#[test]
fn alternate_display_aligns_gutter_pipe() {
    let text = "foo: bar\n\nbaz:\n\n  qux: datetime\n";
    let error = Error::UnsupportedType {
        location: Box::new(Location::in_text(source(), text, Some(5), Some(8), None)),
        found: "datetime",
    };
    let message = format!("{error:#}");
    let source_line = message
        .lines()
        .find(|line| line.contains("qux: datetime"))
        .expect("source line");
    let underline_line = message
        .lines()
        .find(|line| line.contains('^'))
        .expect("underline line");
    let source_pipe = source_line.find('|').expect("source pipe");
    let underline_pipe = underline_line.find('|').expect("underline pipe");
    assert_eq!(source_pipe, underline_pipe);
}
