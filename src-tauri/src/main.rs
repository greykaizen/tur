// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tur_lib::run()
}

// destructors (Drop trait)
// Download for bincode + store
// DM -> close all handles via cancel calls, db conn then drop self

// DM -> <JoinHandle, Watch>
