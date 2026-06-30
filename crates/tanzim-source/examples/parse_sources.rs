use tanzim_source::Source;

fn main() {

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
        println!("{input}");
        match Source::parse(input) {
            Ok(source) => {
                println!("  source: {}", source.source());
                println!("  ignore_errors: {}", source.ignore_errors());
                if source.resource().is_empty() {
                    println!("  resource: (none)");
                } else {
                    println!("  resource: {}", source.resource());
                }
                if source.options().is_empty() {
                    println!("  options: (none)");
                } else {
                    println!("  options: {}", source.options());
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
