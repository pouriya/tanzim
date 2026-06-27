//! Custom loader example.
//!
//! Implements [`Load`] for an in-memory source called `"memory"` and demonstrates
//! the full load flow.
//!
//! Run with:
//!   cargo run -p tanzim-load --example custom_loader

use tanzim_load::{Error, Load, Payload, Source};
use tanzim_source::SourceBuilder;

// ── Custom loader ─────────────────────────────────────────────────────────────

/// Serves fixed in-memory config entries when the source is `"memory"`.
struct MemoryLoader {
    entries: Vec<(String, String, Vec<u8>)>, // (name, format, bytes)
}

impl MemoryLoader {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn add(mut self, name: &str, format: &str, content: &[u8]) -> Self {
        self.entries
            .push((name.to_string(), format.to_string(), content.to_vec()));
        self
    }
}

impl Load for MemoryLoader {
    fn name(&self) -> &str {
        "memory"
    }

    fn supported_source_list(&self) -> Vec<String> {
        vec!["memory".to_string()]
    }

    fn load(&self, source: Source) -> Result<Vec<Payload>, Error> {
        let mut result = Vec::new();
        for index in 0..self.entries.len() {
            let (name, format, bytes) = &self.entries[index];
            result.push(
                Payload {
                    source: source.clone(),
                    name: Some(name.clone()),
                    format: Some(format.clone()),
                    content: bytes.clone(),
                }
                .normalize(),
            );
        }
        Ok(result)
    }
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() {
    let loader = MemoryLoader::new()
        .add("database", "json", br#"{"host":"localhost","port":5432}"#)
        .add("cache", "json", br#"{"host":"127.0.0.1","ttl":300}"#)
        .add("app", "json", br#"{"debug":true,"workers":4}"#);

    let source = SourceBuilder::new().with_source("memory").build().unwrap();

    let payloads = match loader.load(source) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("load error: {e}");
            std::process::exit(1);
        }
    };

    println!("loaded {} entries from memory source:", payloads.len());
    for (index, payload) in payloads.iter().enumerate() {
        println!(
            "  [{index}] name={:?} format={:?} bytes={}",
            payload.name,
            payload.format,
            payload.content.len()
        );
    }
}
