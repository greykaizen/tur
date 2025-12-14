// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Handle early arguments (help, version) before starting GUI
    if tur_lib::args::handle_early_args() {
        return;
    }
    
    tur_lib::run()
}
