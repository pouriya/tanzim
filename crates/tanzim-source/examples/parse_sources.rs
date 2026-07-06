// Example of how to parse a source using the tanzim-source crate.
// Use this example to test the source parsing capabilities of the tanzim-source crate.

use tanzim_source::Source;

fn main() {
    let inputs: Vec<String> = std::env::args().skip(1).collect();
    if inputs.is_empty() {
        eprintln!("Usage: parse_sources <SOURCE> [<SOURCE> ...]");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("\tparse_sources env");
        eprintln!("\tparse_sources 'env(prefix=APP_)'");
        eprintln!("\tparse_sources 'file:/etc/app/config.json'");
        eprintln!("\tparse_sources 'file(on_error=(load=skip)):.env'");
        eprintln!(
            "\tparse_sources 'http(headers=(Authorization=\"TOKEN\"),timeout=3s,on_error=(load=skip)):https://example.com/config.yml'"
        );
        std::process::exit(1);
    }

    let mut failed = false;

    for input in inputs {
        println!("Input: {input}");
        match Source::parse(&input) {
            Ok(source) => {
                println!("\tsource: {}", source.source());
                for stage in [
                    tanzim_source::Stage::Load,
                    tanzim_source::Stage::Parse,
                    tanzim_source::Stage::Validate,
                ] {
                    println!("\t\ton_error[{stage}]: {:?}", source.on_error(stage));
                }
                if source.resource().is_empty() {
                    println!("\tresource: (none)");
                } else {
                    println!("\tresource: {}", source.resource());
                }
                if source.options().is_empty() {
                    println!("\toptions: (none)");
                } else {
                    println!("\toptions: {}", source.options());
                }
            }
            Err(error) => {
                failed = true;
                eprintln!("Could not parse source: {error:#}");
            }
        }
    }

    if failed {
        std::process::exit(1);
    }
}
