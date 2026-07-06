//! Example of how to load configuration from a source using the tanzim-load crate.

use tanzim_load::{Load, Payload, Source};

fn main() {
    let mut source_list = Vec::new();
    for arg in std::env::args().skip(1) {
        match Source::parse(&arg) {
            Ok(source) => source_list.push(source),
            Err(e) => {
                eprintln!("invalid source {arg}: {e:#}");
                std::process::exit(1);
            }
        }
    }
    if source_list.is_empty() {
        eprintln!("Usage: load_config <SOURCE> | [<SOURCE>] ...");
        eprintln!("Examples:");
        eprintln!("\tload_config env");
        eprintln!("\tload_config file:/etc/myapp");
        std::process::exit(1);
    }

    let loader_list: Vec<Box<dyn Load>> = vec![
        Box::new(tanzim_load::env::Env::new()),
        Box::new(tanzim_load::file::File::new()),
        http_loader(),
    ];
    for source in source_list {
        let mut loaded = false;
        for loader in &loader_list {
            let source_name = source.source().to_string();
            if loader.supported_source_list().contains(&source_name) {
                let loader_name = loader.name();
                let payload_list = match loader.load(source.clone()) {
                    Ok(payload_list) => payload_list,
                    Err(e) => {
                        eprintln!("{e:#}");
                        std::process::exit(1);
                    }
                };
                println!("loaded {source} with {loader_name}");
                for (i, payload) in payload_list.iter().enumerate() {
                    println!(
                        "\t[{i}] source_resource={:?} name={:?} format={:?} bytes={}",
                        payload.source.resource(),
                        payload.maybe_name.as_ref().unwrap_or(&"<none>".into()),
                        payload.maybe_format.as_ref().unwrap_or(&"<none>".into()),
                        payload.content.len()
                    );
                }
                loaded = true;
            }
        }
        if !loaded {
            eprintln!("no loader for {source}");
            std::process::exit(1);
        }
    }
}

fn http_loader() -> Box<dyn Load> {
    // Tanzim loader does not have HTTP client
    // The user must provide a closure that implements the HTTP transport
    Box::new(tanzim_load::http::Http::new(Box::new(
        |source, url, headers, duration, insecure| -> Result<Vec<Payload>, String> {
            let mut client = attohttpc::get(url);
            // Add headers to the client
            for (key, value) in headers {
                let header_name = match attohttpc::header::HeaderName::try_from(key.as_str()) {
                    Ok(header_name) => header_name,
                    Err(e) => {
                        return Err(format!("invalid HTTP header name {key:?}: {e}"));
                    }
                };
                client = match client.try_header(header_name, value.as_str()) {
                    Ok(client) => client,
                    Err(e) => {
                        return Err(format!("invalid HTTP header value for {key:?}: {e}"));
                    }
                };
            }
            // Apply timeout and insecure flags
            client = client
                .timeout(duration)
                .danger_accept_invalid_certs(insecure);

            // Send the request
            let response = match client.send() {
                Ok(response) if response.status().is_success() => response,
                Ok(response) => {
                    return Err(format!("HTTP {}", response.status()));
                }
                Err(error) => {
                    return Err(format!("HTTP request failed: {error}"));
                }
            };

            // Extract the name from the last path segment of the URL
            let maybe_name = url.path().split('/').next_back().map(|s| s.to_string());

            // Extract the format from the content-type header (e.g. "application/json; charset=utf-8" -> "json")
            let maybe_format = if let Some(content_type) = response.headers().get("content-type") {
                if let Some((format, _)) = content_type.to_str().unwrap_or_default().split_once(';')
                {
                    if let Some((format, _)) = format.split_once('/') {
                        Some(format.trim().to_string())
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            // Read the response body
            let content = match response.bytes() {
                Ok(content) => content,
                Err(e) => {
                    return Err(format!("HTTP response body read failed: {e}"));
                }
            };

            let payload = Payload {
                source,
                maybe_name,
                maybe_format,
                content,
            };

            Ok(vec![payload])
        },
    )))
}
