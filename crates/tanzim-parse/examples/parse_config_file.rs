// Example of how to parse a config file using the tanzim-parse crate.

use std::{env, fs, path::PathBuf, process};

use tanzim_parse::{Parse, env::Env, json::Json, toml::Toml, yaml::Yaml};
use tanzim_source::SourceBuilder;

fn main() {
    let path = env::args()
        .nth(1)
        .expect("usage: parse_config <config-file>");

    let bytes = fs::read(&path).unwrap_or_else(|_| panic!("failed to read {path}"));

    let src = SourceBuilder::new()
        .with_source("file")
        .with_resource(&path)
        .build()
        .expect("failed to build source");

    let maybe_extension = if let Some(extension) = PathBuf::from(&path).extension() {
        extension.to_str().map(|extension| extension.to_lowercase())
    } else {
        None
    };

    let parser_list: Vec<Box<dyn Parse>> = vec![
        Box::new(Json::new()),
        Box::new(Yaml::new()),
        Box::new(Toml::new()),
        Box::new(Env::new()),
    ];
    let mut picked_by_parser = false;
    let mut parse_error = false;
    // Iterate over the parser list:
    for parser in parser_list {
        let mut parse = false;
        // Check if the extension is supported by the parser
        if let Some(ref extension) = maybe_extension {
            if parser.supported_format_list().contains(extension) {
                parse = true;
            }
        // The file has no extension
        // Give the entire file content to the parser to check if the format is supported:
        } else if let Some(true) = parser.is_format_supported(&bytes) {
            parse = true;
        }
        // If the format is supported, parse the file:
        if parse {
            picked_by_parser = true;
            let parser_name = parser.name();
            match parser.parse(&src, &bytes, &[]) {
                Ok(root) => {
                    println!("parsed using {parser_name} parser: {path}:\n{root:#}\n");
                }
                Err(error) => {
                    eprintln!("failed to parse using {parser_name} parser: {error:#}");
                    parse_error = true;
                }
            }
        }
    }

    if !picked_by_parser {
        eprintln!("no parser picked {path}");
        process::exit(1);
    }
    if parse_error {
        process::exit(1);
    }
    process::exit(0);
}
