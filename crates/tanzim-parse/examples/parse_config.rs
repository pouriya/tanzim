use std::env;
use std::fs;
use std::path::Path;
use std::process;
use tanzim_parse::{Parse, env::Env, json::Json, toml::Toml, yaml::Yaml};
use tanzim_source::SourceBuilder;
use tanzim_value::{LocatedValue, Value};

fn main() {
    let path = match env::args().nth(1) {
        Some(path) => path,
        None => {
            eprintln!("usage: parse_config <config-file>");
            process::exit(1);
        }
    };
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("failed to read {path}: {error}");
            process::exit(1);
        }
    };
    let src = match SourceBuilder::new()
        .with_source("file")
        .with_resource(&path)
        .build()
    {
        Ok(source) => source,
        Err(error) => {
            eprintln!("failed to build source: {error}");
            process::exit(1);
        }
    };
    let format = match Path::new(&path)
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some("json") => "json",
        Some("yaml") | Some("yml") => "yaml",
        Some("toml") => "toml",
        Some("env") => "env",
        _ => {
            eprintln!(
                "unsupported config file extension: {}",
                Path::new(&path).display()
            );
            eprintln!("supported: .json .yaml .yml .toml .env");
            process::exit(1);
        }
    };
    let result = match format {
        "json" => Json::new().parse(&src, &bytes),
        "yaml" => Yaml::new().parse(&src, &bytes),
        "toml" => Toml::new().parse(&src, &bytes),
        "env" => Env::new().parse(&src, &bytes),
        _ => unreachable!(),
    };
    match result {
        Ok(root) => {
            println!("parsed {path} ({format}):");
            print_located(&root, 0);
        }
        Err(error) => {
            eprintln!("{error:#}");
            process::exit(1);
        }
    }
}

fn print_located(located: &LocatedValue, indent: usize) {
    let prefix = "  ".repeat(indent);
    let source = located.location.to_string();
    let location = match (located.location.line, located.location.column) {
        (Some(line), Some(column)) => format!("@ {source}:{line}:{column}"),
        (Some(line), None) => format!("@ {source}:{line}"),
        _ => format!("@ {source}"),
    };
    match &located.value {
        Value::Bool(value) => println!("{prefix}{value} {location}"),
        Value::Int(value) => println!("{prefix}{value} {location}"),
        Value::Float(value) => println!("{prefix}{value} {location}"),
        Value::String(value) => println!("{prefix}{value:?} {location}"),
        Value::List(values) => {
            println!("{prefix}[ {location}");
            for value in values {
                print_located(value, indent + 1);
            }
            println!("{prefix}]");
        }
        Value::Map(map) => {
            println!("{prefix}{{ {location}");
            for (key, value) in map.entries() {
                let key_prefix = "  ".repeat(indent + 1);
                println!("{key_prefix}{key:?}:");
                print_located(value, indent + 2);
            }
            println!("{prefix}}}");
        }
        Value::Null => println!("{prefix}null {location}"),
        Value::Comment(value) => println!("{prefix}{value} {location}"),
    }
}
