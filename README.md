<link href="https://fonts.googleapis.com/css2?family=Modak&display=swap" rel="stylesheet">

<div align="center">
  <img src="/src-tauri/icons/icon.png" alt="tur logo" width="80" style="display: inline-block; vertical-align: middle;"> 
  <span style="font-family: 'Modak', sans-serif; font-size: 3.5em; color: transparent; -webkit-text-stroke: 2px #000; text-stroke: 2px #000; display: inline-block; vertical-align: middle; margin-left: 15px; letter-spacing: 0.05em;">tur</span>
</div>

<p align="center">A fast, modern download manager for desktop</p>

<p align="center">
  <img src="https://img.shields.io/badge/License-Apache%202.0-blue.svg" alt="License">
  <img src="https://img.shields.io/badge/version-0.1.0-green.svg" alt="Version">
</p>

---

**tur** (pronounced "toor", طور) is a download manager that actually gets out of your way. Built with Tauri and React, it uses multi-threaded connections to speed up your downloads while keeping everything organized in a clean, distraction-free interface. No bloat, no unnecessary features—just fast, reliable downloads.

## Features

- **Multi-threaded downloads** – Split files into chunks for significantly faster speeds
- **Full download control** – Pause, resume, or cancel anytime without losing progress
- **Smart queue management** – Organize and prioritize what downloads first
- **Complete history** – Never lose track of what you've downloaded
- **Drag & drop** – Just drop a URL or file to start downloading
- **Flexible settings** – Customize download locations, thread counts, and more
- **Theme support** – Light, dark, or follow your system preference
- **Desktop notifications** – Get notified when downloads finish or hit issues
- **Cross-platform** – Works seamlessly on Windows, macOS, and Linux

## Development

```bash
# Clone the repository
git clone https://github.com/yourusername/tur.git
cd tur

# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

## Contributing

Contributions are welcome! Feel free to open issues or submit pull requests.

## License

Licensed under the Apache License 2.0. See [LICENSE](LICENSE) for details.
