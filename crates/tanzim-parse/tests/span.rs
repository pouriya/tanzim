use tanzim_parse::span::{char_count, is_single_line, line_column, line_column_from_line};

#[test]
fn line_column_counts_utf8_chars() {
    let text = "α: 1\nb: 2";
    let (line, column) = line_column(text, 0);
    assert_eq!(line, 1);
    assert_eq!(column, 1);
    let (line, column) = line_column(text, text.find('\n').unwrap() + 1);
    assert_eq!(line, 2);
    assert_eq!(column, 1);
}

#[test]
fn is_single_line_detects_newlines() {
    assert!(is_single_line(b"hello"));
    assert!(!is_single_line(b"a\nb"));
}

#[test]
fn char_count_handles_ranges_and_empty_spans() {
    let text = "αβ";
    assert_eq!(char_count(text, 0, text.len()), 2);
    assert_eq!(char_count(text, 1, 1), 0);
}

#[test]
fn line_column_from_line_returns_column_on_target_line() {
    let text = "first\nsecond";
    let column = line_column_from_line(text, 2, text.find("second").unwrap());
    assert_eq!(column, 1);
}
