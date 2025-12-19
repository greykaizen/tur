//! CLI argument parsing and TUI mode support
//!
//! Default: Terminal mode with progress bars
//! Use --gui to open graphical interface

use std::env;
use std::path::PathBuf;

/// Parsed command-line arguments
#[derive(Debug, Clone)]
pub struct AppArgs {
    /// Run in GUI mode (default: false = terminal mode)
    pub gui: bool,
    /// Interactive TUI with progress bars (default for terminal mode)
    pub interactive: bool,
    /// Quiet mode - no output, exit code only
    pub quiet: bool,
    /// Enable debug logging
    pub debug: bool,
    /// Start GUI minimized to tray
    pub minimized: bool,
    /// URLs to download
    pub urls: Vec<String>,
    /// File containing URLs (one per line)
    pub file: Option<PathBuf>,
    /// Output directory
    pub output: Option<PathBuf>,
    /// Connections per download (overrides settings)
    pub connections: Option<u8>,
    /// Speed limit (e.g., "1M", "500K")
    pub limit: Option<String>,
    /// Deep link URL (tur://...)
    pub deep_link: Option<String>,
    /// Print help
    pub help: bool,
    /// Print version
    pub version: bool,
}

impl Default for AppArgs {
    fn default() -> Self {
        Self {
            gui: false,
            interactive: true, // Default to interactive in terminal mode
            quiet: false,
            debug: false,
            minimized: false,
            urls: Vec::new(),
            file: None,
            output: None,
            connections: None,
            limit: None,
            deep_link: None,
            help: false,
            version: false,
        }
    }
}

impl AppArgs {
    /// Parse command-line arguments
    pub fn parse() -> Self {
        let args: Vec<String> = env::args().collect();
        Self::parse_from_vec(&args[1..]) // Skip program name
    }

    /// Parse from a slice of arguments
    pub fn parse_from_vec(args: &[String]) -> Self {
        let mut parsed = AppArgs::default();
        let mut i = 0;

        while i < args.len() {
            let arg = args[i].as_str();

            match arg {
                // Mode flags
                "--gui" | "-g" => parsed.gui = true,
                "--interactive" | "-i" => parsed.interactive = true,
                "--quiet" | "-q" => {
                    parsed.quiet = true;
                    parsed.interactive = false;
                }

                // App flags
                "--minimized" | "-m" => parsed.minimized = true,
                "--debug" | "-d" => parsed.debug = true,
                "--help" | "-h" => parsed.help = true,
                "--version" | "-V" => parsed.version = true,

                // Options with values
                "--file" | "-f" => {
                    i += 1;
                    if i < args.len() {
                        parsed.file = Some(PathBuf::from(&args[i]));
                    }
                }
                "--output" | "-o" => {
                    i += 1;
                    if i < args.len() {
                        parsed.output = Some(PathBuf::from(&args[i]));
                    }
                }
                "--connections" | "-c" => {
                    i += 1;
                    if i < args.len() {
                        parsed.connections = args[i].parse().ok();
                    }
                }
                "--limit" | "-l" => {
                    i += 1;
                    if i < args.len() {
                        parsed.limit = Some(args[i].clone());
                    }
                }

                // Deep link
                arg if arg.starts_with("tur://") => {
                    parsed.deep_link = Some(arg.to_string());
                }

                // URLs (anything starting with http/https or looks like a URL)
                arg if arg.starts_with("http://") || arg.starts_with("https://") => {
                    parsed.urls.push(arg.to_string());
                }

                // Unknown - could be a URL without scheme or unknown flag
                _ => {
                    // If it doesn't start with -, treat as potential URL
                    if !arg.starts_with('-') {
                        parsed.urls.push(arg.to_string());
                    }
                    // Unknown flags are ignored
                }
            }

            i += 1;
        }

        // If no URLs and no special flags, default to GUI mode
        if parsed.urls.is_empty()
            && parsed.file.is_none()
            && parsed.deep_link.is_none()
            && !parsed.help
            && !parsed.version
        {
            parsed.gui = true;
        }

        parsed
    }

    /// Parse speed limit string to bytes per second
    pub fn parse_speed_limit(&self) -> Option<u64> {
        self.limit.as_ref().map(|s| parse_size(s))
    }

    /// Print help message with emojis
    pub fn print_help() {
        let help = r#"
🚀 tur - A fast, modern download manager

USAGE:
    tur [OPTIONS] [URLS...]

MODE:
    (default)           Terminal mode with progress bars
    -g, --gui           Open graphical interface
    -q, --quiet         Silent mode (exit code only)
    -i, --interactive   Force interactive TUI (default in terminal)

OPTIONS:
    -f, --file <PATH>       Read URLs from file (one per line)
    -o, --output <DIR>      Download to specific directory
    -c, --connections <N>   Connections per download (1-64)
    -l, --limit <SPEED>     Speed limit (e.g., 1M, 500K, 2G)
    -m, --minimized         Start GUI minimized to tray
    -d, --debug             Enable debug logging
    -h, --help              Print this help message
    -V, --version           Print version information

EXAMPLES:
    tur https://example.com/file.zip          📥 Download in terminal
    tur -o ~/Downloads file1.zip file2.zip    📁 Download to directory
    tur -f urls.txt -l 2M                     📋 Batch with speed limit
    tur -q https://example.com/file.zip       🤫 Silent download
    tur --gui                                 🖥️  Open GUI
    tur                                       🖥️  Open GUI (no URLs)
"#;
        println!("{}", help);
    }

    /// Print version with emoji
    pub fn print_version() {
        println!("🚀 tur v{}", env!("CARGO_PKG_VERSION"));
    }

    /// Check if we should run in terminal mode
    pub fn is_terminal_mode(&self) -> bool {
        !self.gui && (self.has_downloads() || self.help || self.version)
    }

    /// Check if we have downloads to process
    pub fn has_downloads(&self) -> bool {
        !self.urls.is_empty() || self.file.is_some() || self.deep_link.is_some()
    }
}

/// Parse size string (e.g., "1M", "500K", "2G") to bytes
fn parse_size(s: &str) -> u64 {
    let s = s.trim().to_uppercase();
    let (num, mult) = if s.ends_with('G') {
        (&s[..s.len() - 1], 1024 * 1024 * 1024)
    } else if s.ends_with('M') {
        (&s[..s.len() - 1], 1024 * 1024)
    } else if s.ends_with('K') {
        (&s[..s.len() - 1], 1024)
    } else {
        (s.as_str(), 1)
    };
    num.parse::<u64>().unwrap_or(0) * mult
}

/// Handle early args that exit before app starts
pub fn handle_early_args() -> bool {
    let args = AppArgs::parse();

    if args.help {
        AppArgs::print_help();
        return true;
    }

    if args.version {
        AppArgs::print_version();
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("1K"), 1024);
        assert_eq!(parse_size("1M"), 1024 * 1024);
        assert_eq!(parse_size("1G"), 1024 * 1024 * 1024);
        assert_eq!(parse_size("500K"), 500 * 1024);
        assert_eq!(parse_size("2m"), 2 * 1024 * 1024);
    }

    #[test]
    fn test_gui_default_no_urls() {
        let args = AppArgs::parse_from_vec(&[]);
        assert!(args.gui);
    }

    #[test]
    fn test_terminal_with_url() {
        let args = AppArgs::parse_from_vec(&["https://example.com/file.zip".into()]);
        assert!(!args.gui);
        assert_eq!(args.urls.len(), 1);
    }
}
