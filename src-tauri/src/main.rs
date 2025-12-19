// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tur_lib::args::AppArgs;

fn main() {
    // Handle early arguments (help, version) before starting app
    if tur_lib::args::handle_early_args() {
        return;
    }

    let args = AppArgs::parse();

    if args.is_terminal_mode() {
        // Terminal mode: download with real progress bars
        run_terminal_mode(args);
    } else {
        // GUI mode: start Tauri app
        tur_lib::run();
    }
}

/// Run downloads in terminal mode with TUI progress bars
fn run_terminal_mode(args: AppArgs) {
    use console::Style;
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    // Collect URLs from args and file
    let mut urls: Vec<String> = args.urls.clone();

    // Read URLs from file if provided
    if let Some(file_path) = &args.file {
        match File::open(file_path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                for line in reader.lines().flatten() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() && !trimmed.starts_with('#') {
                        urls.push(trimmed.to_string());
                    }
                }
            }
            Err(e) => {
                let red = Style::new().red().bold();
                eprintln!("❌ {} Failed to read file: {}", red.apply_to("Error:"), e);
                std::process::exit(1);
            }
        }
    }

    // Handle deep link
    if let Some(deep_link) = &args.deep_link {
        if let Some(url) = parse_deep_link(deep_link) {
            urls.push(url);
        }
    }

    if urls.is_empty() {
        let red = Style::new().red().bold();
        eprintln!("❌ {} No URLs provided", red.apply_to("Error:"));
        std::process::exit(1);
    }

    // Clear screen and print header unless quiet
    if !args.quiet {
        tur_lib::cli::clear_and_header();
    }

    // Run async downloads
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create runtime");

    let results = rt.block_on(tur_lib::cli::run_downloads(&args, urls));

    // Print summary
    if !args.quiet {
        let success_count = results.iter().filter(|r| r.success).count();
        let fail_count = results.len() - success_count;

        println!();
        if fail_count == 0 {
            let green = Style::new().green().bold();
            println!(
                "✅ {} {} download(s) complete!",
                green.apply_to("Success:"),
                success_count
            );
        } else {
            let yellow = Style::new().yellow().bold();
            println!(
                "⚠️  {} {} succeeded, {} failed",
                yellow.apply_to("Done:"),
                success_count,
                fail_count
            );
        }
        println!();
    }

    // Exit with error if any failed
    if results.iter().any(|r| !r.success) {
        std::process::exit(1);
    }
}

/// Parse tur:// deep link to extract URL
fn parse_deep_link(deep_link: &str) -> Option<String> {
    if let Some(query) = deep_link.strip_prefix("tur://download?") {
        for param in query.split('&') {
            if let Some(url) = param.strip_prefix("url=") {
                return Some(urlencoding_decode(url));
            }
        }
    }
    None
}

/// Simple URL decoding
fn urlencoding_decode(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    result
}
