# Backlog - Future Work

## Frontend Tasks
- [ ] Pop-up window on download start (show destination picker)
- [ ] Pop-up window on download complete (open directory / close button)
- [ ] Pop-up window on DeepLink arrival (wake/show app)
- [ ] Clipboard paste icon for quick URL input
- [ ] File conflict dialog (rename/overwrite/skip)
- [ ] Auto-resume notification on startup

## Frontend Settings UI
- [ ] NetworkConfig UI (user agent dropdown: Chrome/Firefox/Edge/Safari/Custom)
- [ ] NetworkConfig UI (connect timeout, read timeout, retry count, retry delay)
- [ ] NetworkConfig UI (allow insecure certificates toggle)
- [ ] ProxyConfig UI (enable, type dropdown, host, port)
- [ ] ProxyConfig UI (auth toggle, username, password fields)
- [ ] Max concurrent downloads setting
- [ ] Category-based download folders by file type
- [ ] Notification sound toggle

## Backend Settings
- [x] Add network.* and proxy.* update handlers in store.rs ✅
- [x] Settings validation (num_threads 1-64, timeouts > 0, etc.) ✅
- [ ] Settings schema migration for version upgrades

## Backend Features
- [ ] Browser extension integration (intercept downloads, capture cookies)
- [ ] Scheduler (download at specific times only)
- [ ] Mirror support (alternate URLs on failure)
- [ ] Checksum verification (MD5/SHA256 after completion)
- [ ] Link refresh (detect 403/410, prompt user for new URL)
- [ ] Session-based links (cookie management via extension)

## CLI / TUI Mode
- [ ] `-i, --interactive` flag for TUI mode with indicatif
- [ ] `-f, --file <path>` read URLs from txt/csv/json
- [ ] `-o, --output <dir>` override download location
- [ ] `--add <url>` add to running instance via IPC
- [ ] Progress bars per download with speed/ETA
- [ ] Ctrl+C graceful shutdown

## Deferred Backend
- [ ] Speed limiting implementation (`ControlCommand::SpeedLimit`)
