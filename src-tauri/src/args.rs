use std::env;

#[derive(Debug, Clone)]
pub struct AppArgs {
    pub minimized: bool,
    pub debug: bool,
    pub deep_link: Option<String>,
    pub help: bool,
    pub version: bool,
}

impl Default for AppArgs {
    fn default() -> Self {
        Self {
            minimized: false,
            debug: false,
            deep_link: None,
            help: false,
            version: false,
        }
    }
}

impl AppArgs {
    pub fn parse() -> Self {
        let args: Vec<String> = env::args().collect();
        let mut parsed = AppArgs::default();
        
        let mut i = 1; // Skip program name
        while i < args.len() {
            match args[i].as_str() {
                "--minimized" | "-m" => {
                    parsed.minimized = true;
                }
                "--debug" | "-d" => {
                    parsed.debug = true;
                }
                "--help" | "-h" => {
                    parsed.help = true;
                }
                "--version" | "-v" => {
                    parsed.version = true;
                }
                arg if arg.starts_with("tur://") => {
                    parsed.deep_link = Some(arg.to_string());
                }
                _ => {
                    // Unknown argument, ignore for now
                }
            }
            i += 1;
        }
        
        parsed
    }
    
    pub fn parse_from_vec(args: &[String]) -> Self {
        let mut parsed = AppArgs::default();
        
        for arg in args {
            match arg.as_str() {
                "--minimized" | "-m" => {
                    parsed.minimized = true;
                }
                "--debug" | "-d" => {
                    parsed.debug = true;
                }
                "--help" | "-h" => {
                    parsed.help = true;
                }
                "--version" | "-v" => {
                    parsed.version = true;
                }
                arg if arg.starts_with("tur://") => {
                    parsed.deep_link = Some(arg.to_string());
                }
                _ => {
                    // Unknown argument, ignore for now
                }
            }
        }
        
        parsed
    }
    
    pub fn print_help() {
        println!("tur - A fast, modern download manager");
        println!();
        println!("USAGE:");
        println!("    tur [OPTIONS] [URL]");
        println!();
        println!("OPTIONS:");
        println!("    -m, --minimized    Start minimized to system tray");
        println!("    -d, --debug        Enable debug logging");
        println!("    -h, --help         Print this help message");
        println!("    -v, --version      Print version information");
        println!();
        println!("ARGUMENTS:");
        println!("    URL                Deep link URL (tur://...)");
        println!();
        println!("EXAMPLES:");
        println!("    tur --minimized");
        println!("    tur 'tur://download?url=https://example.com/file.zip'");
    }
    
    pub fn print_version() {
        println!("tur {}", env!("CARGO_PKG_VERSION"));
    }
}

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