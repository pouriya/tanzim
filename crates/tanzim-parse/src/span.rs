/// Whether `bytes` contain no newline (single-line document).
pub fn is_single_line(bytes: &[u8]) -> bool {
    for &byte in bytes {
        if byte == b'\n' {
            return false;
        }
    }
    true
}

/// 1-based line and UTF-8 character column at `byte_offset` in `text`.
pub fn line_column(text: &str, byte_offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut column = 1usize;
    let mut index = 0usize;
    while index < byte_offset && index < text.len() {
        let ch = text[index..].chars().next().expect("valid utf-8");
        let ch_len = ch.len_utf8();
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
        index += ch_len;
    }
    (line, column)
}

/// UTF-8 character count in `text[start_byte..end_byte]`.
pub fn char_count(text: &str, start_byte: usize, end_byte: usize) -> usize {
    let end_byte = end_byte.min(text.len());
    if start_byte >= end_byte {
        return 0;
    }
    let mut count = 0usize;
    let mut index = start_byte;
    while index < end_byte {
        let ch = text[index..].chars().next().expect("valid utf-8");
        count += 1;
        index += ch.len_utf8();
    }
    count
}

/// 1-based line and UTF-8 character column at `byte_offset` in `text`.
pub fn line_column_from_line(text: &str, line_number: usize, byte_offset: usize) -> usize {
    let mut line = 1usize;
    let mut column = 1usize;
    let mut index = 0usize;
    while index < text.len() {
        if line == line_number && index >= byte_offset {
            return column;
        }
        let ch = text[index..].chars().next().expect("valid utf-8");
        let ch_len = ch.len_utf8();
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
        index += ch_len;
    }
    if line == line_number { column } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
