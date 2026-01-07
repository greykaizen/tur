# Backlog - Future Work

## ✅ Recently Completed
- [x] Welcome screen toggle in Appearance settings ✅
- [x] Empty state management refactored to context-based ✅
- [x] Auto-rename backend for file conflicts ✅
- [x] Settings validation (num_threads 1-64, timeouts > 0, etc.) ✅
- [x] Network.* and proxy.* update handlers in store.rs ✅
- [x] `request_shutdown` Tauri command (stop all downloads, exit) ✅
- [x] System tray implementation with menu (Show/Quit) ✅
- [x] Window close behavior based on settings ✅
- [x] Single-instance app wake-up (double-click binary shows window) ✅
- [x] Multi-thread indicator (Connections: N threads) ✅
- [x] Resume indicator (Yes/No with muted red for No) ✅
- [x] Variance-adaptive EWMA speed smoothing ✅
- [x] IDM-style time format (5 min 35 sec) ✅
- [x] Version sync across Cargo.toml, package.json, tauri.conf.json ✅
- [x] Dynamic version display in About page ✅
- [x] Ctrl+Q/Quit actually exits app (not tray) ✅
- [x] Browser extension scaffold (manifest, content script, popup) ✅

---

## 🎯 Current Sprint: Video Download Architecture

### Phase 1: Extension → App Communication
- [ ] Fix deep link reliability on Linux (URL length issue)
- [ ] Extension sends short URL: `tur://youtube/VIDEO_ID` or `tur://video?url=...`
- [ ] App receives and parses deep link properly
- [ ] Test with YouTube and generic video URLs

### Phase 2: Download Dialog Popup Window
- [ ] Create new Tauri window for download dialog (popup, stays on top)
- [ ] Dialog layout:
  - Thumbnail preview (for video sites)
  - URL display
  - Quality/format selector (populated by yt-dlp)
  - Audio track selector
  - Save location picker with category
  - "Remember for this site" checkbox
- [ ] Category system (Video, Music, Documents, etc.)
- [ ] Remember category preferences per domain

### Phase 3: yt-dlp Integration
- [ ] Download yt-dlp binary on first use (not bundled to save size)
- [ ] Settings: custom yt-dlp path option
- [ ] Auto-update check for yt-dlp (weekly or on startup)
- [ ] Run `yt-dlp -J <url>` to get available formats
- [ ] Parse JSON response for:
  - Video formats (height, fps, codec, size)
  - Audio formats (quality, codec, size)
  - Thumbnail URL
  - Title, duration, uploader
- [ ] Handle yt-dlp errors gracefully

### Phase 4: ffmpeg Integration
- [ ] Download ffmpeg binary on first use (or detect system install)
- [ ] Settings: custom ffmpeg path option
- [ ] Merge video + audio for DASH/HLS formats
- [ ] Progress indication during merge
- [ ] Handle merge errors with retry option

### Phase 5: Smart Download Routing
- [ ] **Progressive formats**: Single-threaded direct download
- [ ] **DASH/HLS formats**:
  - Download video segments (multi-worker)
  - Download audio separately
  - Temp directory for parts
  - ffmpeg merge when complete
  - Move to final destination
  - Cleanup temp files
- [ ] Audio-only download option (convert to MP3)
- [ ] Subtitle download option (.srt)

### Phase 6: Extension Polish
- [ ] Download button on any `<video>` element
- [ ] Button positioned in player controls when possible
- [ ] Fallback: button outside video frame (top-right)
- [ ] Playlist detection (YouTube playlist → queue all)
- [ ] Context menu "Download with tur"

---

## High Priority (After Video Architecture)

### Download State Persistence
- [ ] Per-download `.tur.meta` JSON file
- [ ] Load incomplete downloads on startup
- [ ] Resume from last known byte position
- [ ] Auto-resume prompt on startup

### UI Polish
- [ ] Tray icon badge for active downloads count
- [ ] Blue range progress bar (IDM-style worker segments)
- [ ] Notification on complete (when in tray/background)

---

## Frontend Settings UI
- [ ] NetworkConfig UI (user agent, timeouts, retry)
- [ ] ProxyConfig UI (enable, type, host, port, auth)
- [ ] Max concurrent downloads setting
- [ ] Category-based download folders by file type
- [ ] Notification sound toggle
- [ ] yt-dlp path setting
- [ ] ffmpeg path setting

## Backend Features
- [ ] Speed limiting implementation (`ControlCommand::SpeedLimit`)
- [ ] Dynamic tray icon removal/creation on settings change
- [ ] Browser extension native messaging (fallback for deep link)
- [ ] Scheduler (download at specific times only)
- [ ] Mirror support (alternate URLs on failure)
- [ ] Checksum verification (MD5/SHA256 after completion)

## CLI / TUI Mode
- [ ] `-i, --interactive` flag for TUI mode
- [ ] `-f, --file <path>` read URLs from txt/csv/json
- [ ] `-o, --output <dir>` override download location
- [ ] Progress bars per download with speed/ETA

---

## Architecture Reference

### Video Download Flow
```
Extension                    Tauri App
─────────                    ─────────
Detect video                     
Click button                     
   ↓                             
tur://youtube/VIDEO_ID  →    Open Download Dialog (popup)
                                  ↓
                             yt-dlp -J to get formats
                                  ↓
                             Show format picker
                             User picks quality
                                  ↓
                             If Progressive:
                               Direct download
                             If DASH/HLS:
                               Multi-segment download
                               ffmpeg merge
                               Cleanup temp
```

### Download Dialog Window
```
┌──────────────────────────────────────────────┐
│ [Thumbnail] Video Title                      │
│ URL: https://youtube.com/watch?v=...         │
│                                              │
│ Quality: [1080p ▼]  Audio: [Best ▼]         │
│ Format: [MP4 ▼]                             │
│                                              │
│ Save to: [~/Videos/YouTube] [Browse]         │
│ Category: [Video ▼]  ☑ Remember for site    │
│                                              │
│           [Cancel]  [Download]               │
└──────────────────────────────────────────────┘
```
