//! Custom parser example.
//!
//! Implements [`Deserialize`] for a simple `key=value` format (one entry per line,
//! `#` comments) and runs it against inline bytes.
//!
//! Run with:
//!   cargo run -p tanzim-parse --example custom_parser

use tanzim_parse::{Deserialize, Error, LocatedValue, Value};
use tanzim_value::{Location, Map};

// ── Custom parser ─────────────────────────────────────────────────────────────

/// Parses `key=value` lines. Lines starting with `#` are comments.
struct KvParser;

impl Deserialize for KvParser {
    fn name(&self) -> &str {
        "kv"
    }

    fn supported_format_list(&self) -> Vec<String> {
        vec!["kv".to_string(), "properties".to_string()]
    }

    fn is_format_supported(&self, bytes: &[u8]) -> Option<bool> {
        // Heuristic: looks like kv if the first non-comment line contains `=`
        let text = match std::str::from_utf8(bytes) {
            Ok(t) => t,
            Err(_) => return Some(false),
        };
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            return Some(trimmed.contains('='));
        }
        None
    }

    fn parse(&self, source: &str, resource: &str, bytes: &[u8]) -> Result<LocatedValue, Error> {
        let text = match std::str::from_utf8(bytes) {
            Ok(t) => t,
            Err(_) => {
                return Err(Error::InvalidUtf8 {
                    location: Location::at(source, resource, None, None, None),
                });
            }
        };

        let mut map = Map::new();
        for (line_idx, line) in text.lines().enumerate() {
            let line_number = line_idx + 1;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            match trimmed.split_once('=') {
                None => {
                    return Err(Error::Parse {
                        text: text.to_string(),
                        location: Some(Location::at(
                            source,
                            resource,
                            Some(line_number),
                            None,
                            None,
                        )),
                        message: format!(
                            "line {line_number}: expected `key=value`, got: {trimmed:?}"
                        ),
                    });
                }
                Some((key, val)) => {
                    let loc = Location::at(source, resource, Some(line_number), None, None);
                    map.insert(
                        key.trim().to_string(),
                        LocatedValue {
                            value: Value::String(val.trim().to_string()),
                            location: loc,
                        },
                    );
                }
            }
        }

        let root_loc = Location::at(source, resource, None, None, None);
        Ok(LocatedValue {
            value: Value::Map(map),
            location: root_loc,
        })
    }
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() {
    let input = b"# Database settings\nhost = localhost\nport = 5432\n\n# Pool\npool_size = 10\n";

    let parser = KvParser;
    println!("parser name  : {}", parser.name());
    println!("formats      : {:?}", parser.supported_format_list());
    println!("auto-detect  : {:?}", parser.is_format_supported(input));
    println!();

    let value = match parser.parse("custom", "db.kv", input) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("parse error: {e:#}");
            std::process::exit(1);
        }
    };

    println!("parsed value : {value}");
    println!();

    let map = value.value.as_map().unwrap();
    for (key, lv) in map.entries() {
        println!("  {key:15} = {}  (at {})", lv.value, lv.location);
    }

    // Demonstrate error reporting
    let bad_input = b"host = localhost\nno-equals-here\nport = 5432\n";
    if let Err(e) = parser.parse("custom", "bad.kv", bad_input) {
        println!();
        println!("parse error (single-line) : {e}");
        println!("parse error (with snippet):\n{e:#}");
    }
}
