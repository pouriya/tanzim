use tanzim_source::{OptionValue, Source};

fn main() {
    fn print_option_value(key: &str, value: &OptionValue, indent: usize) {
        let pad = " ".repeat(indent);
        match value {
            OptionValue::Bool(value) => println!("{pad}{key}: bool = {value}"),
            OptionValue::Integer(value) => println!("{pad}{key}: integer = {value}"),
            OptionValue::Float(value) => println!("{pad}{key}: float = {value}"),
            OptionValue::String(value) => println!("{pad}{key}: string = {value:?}"),
            OptionValue::List(values) => {
                println!("{pad}{key}: list[{len}]", len = values.len());
                for (index, item) in values.iter().enumerate() {
                    print_option_value(&format!("[{index}]"), item, indent + 2);
                }
            }
            OptionValue::Map(options) => {
                println!("{pad}{key}: map{{{len}}}", len = options.len());
                for (inner_key, inner_value) in options.iter() {
                    print_option_value(inner_key, inner_value, indent + 2);
                }
            }
        }
    }

    let inputs: Vec<String> = std::env::args().skip(1).collect();
    if inputs.is_empty() {
        eprintln!("Usage: parse_sources <SOURCE> [<SOURCE> ...]");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  parse_sources env");
        eprintln!("  parse_sources 'env(prefix=APP_)'");
        eprintln!("  parse_sources 'file:/etc/app/config.json'");
        eprintln!("  parse_sources 'file?:.env'");
        eprintln!(
            "  parse_sources 'http(headers=(Authorization=\"TOKEN\"),timeout=3s)?:https://example.com/config.yml'"
        );
        std::process::exit(1);
    }

    let mut failed = false;

    for (index, input) in inputs.iter().enumerate() {
        if index > 0 {
            println!();
        }
        println!("[{index}] input: {input}");

        match Source::parse(input) {
            Ok(source) => {
                println!("  source: {}", source.source());
                println!("  skip_errors: {}", source.skip_errors());
                println!("  resource_colon: {}", source.resource_colon());
                println!("  resource: {:?}", source.resource());
                println!("  canonical: {source}");

                if source.options().is_empty() {
                    println!("  options: (none)");
                } else {
                    println!("  options:");
                    for (key, value) in source.options().iter() {
                        print_option_value(key, value, 2);
                    }
                }
            }
            Err(error) => {
                failed = true;
                eprintln!("error:{error:#}");
            }
        }
    }

    if failed {
        std::process::exit(1);
    }
}
