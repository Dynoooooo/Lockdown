# Lockdown

Remote device control daemon for Windows. Runs on a PC and exposes a mobile-first web UI you control from your phone over Tailscale.

All system interaction is native Win32 FFI — no PowerShell, no .NET, no child processes. Everything runs on Rust-owned threads.

## Features

- **App Blocking** — Kill processes by name (e.g. `discord.exe`, `chrome.exe`). Polls every 3 seconds.
- **Web Filtering** — Block domains via the Windows hosts file. Automatically blocks `www.` variants. Includes one-tap templates for common categories (social media, pornography, video streaming, gaming, news, shopping, gambling, dating, Reddit/forums).
- **Screen Lock** — Fullscreen overlay window with customizable text and visual templates (Minimal, Warning, Elegant, Terminal). Low-level keyboard hook blocks Alt+Tab, Alt+F4, Win key, and other escape shortcuts. Taskbar is hidden and Task Manager is disabled via registry while locked. Unlock is instant — the window runs on a Rust thread and is destroyed via `WM_QUIT`.
- **Screenshots** — Capture the PC screen on demand or auto-capture every 5 seconds. PNG-compressed for fast transfer. Tap to view fullscreen.
- **Schedules** — Time-based rules that activate features on specific days and time ranges. Supports overnight ranges (e.g. 22:00–06:00).
- **Watchdog** — Configurable auto-unlock timeout. If your phone loses connection while the screen is locked, the system automatically unlocks after the set period (off, 5 min, 10 min, 30 min, or 1 hr).
- **Crash Recovery** — On startup, the daemon unconditionally restores the taskbar and re-enables Task Manager, recovering cleanly from any prior crash.
- **Remote Web UI** — Mobile-first control panel. No external dependencies — the entire UI is embedded in the binary.
- **Password Auth** — Argon2id-hashed password with session tokens.

## Requirements

- **Windows 10/11** (the target machine)
- **Rust toolchain** — install from https://rustup.rs
- **Administrator privileges** — required for hosts file editing, taskbar control, registry access, and screenshots
- **Tailscale** — so your phone can reach the PC (https://tailscale.com)

## Build

```
cargo build --release
```

The binary will be at `target\release\lockdown.exe`.

## First Run

```powershell
.\lockdown.exe
```

On first run it will prompt you to set a password. This is what you'll enter on your phone to authenticate.

The config is saved to `lockdown_config.json` in the working directory. You can also pass a custom path:

```powershell
.\lockdown.exe C:\path\to\my_config.json
```

## Connecting from Your Phone

The recommended approach is **Tailscale Funnel**, which routes traffic through Tailscale's relay servers with end-to-end encryption:

```powershell
tailscale funnel 7878
```

This gives you a public `https://<machine-name>.<tailnet>.ts.net` URL that works from your phone's browser.

To keep the funnel running after you close the terminal:

```powershell
tailscale funnel --bg 7878
```

### Alternative: Direct Tailscale Connection

If direct device-to-device connectivity works in your network, you can skip funnel and access the UI at:

```
http://<tailscale-ip>:7878
```

You may need to add a Windows Firewall rule:

```powershell
New-NetFirewallRule -DisplayName "Lockdown" -Direction Inbound -LocalPort 7878 -Protocol TCP -Action Allow
```

## Running as Administrator

For all features to work, run from an elevated prompt:

```powershell
# Right-click PowerShell → Run as Administrator
cd C:\path\to\lockdown
.\target\release\lockdown.exe
```

## Auto-Start on Boot

Create a batch file (`start-lockdown.bat`):

```bat
start "" "C:\path\to\lockdown.exe"
timeout /t 3
tailscale funnel --bg 7878
```

Then create a scheduled task:

```powershell
schtasks /create /tn "Lockdown" /tr "C:\path\to\start-lockdown.bat" /sc onlogon /rl highest /f
```

## Changing the Port

Edit `lockdown_config.json` and change `listen_port`:

```json
{
  "listen_port": 9090
}
```

## Architecture

```
lockdown/
├── src/
│   ├── main.rs        — Entry point, config loading, server setup, startup cleanup
│   ├── state.rs       — Shared state, config structs, persistence
│   ├── auth.rs        — Argon2 password hashing, session tokens
│   ├── api.rs         — HTTP route handlers (axum)
│   ├── win32.rs       — All Win32 FFI declarations and safe wrappers
│   ├── blocker.rs     — Process monitoring and termination loop
│   ├── filter.rs      — Hosts file management for domain blocking
│   ├── locker.rs      — Fullscreen overlay window + keyboard hook on a Rust thread
│   ├── screenshot.rs  — Screen capture via native BitBlt, PNG-encoded
│   ├── scheduler.rs   — Time-based rule evaluation loop
│   └── watchdog.rs    — Heartbeat monitor, auto-unlock on timeout
├── static/
│   └── index.html     — Embedded mobile-first web UI (no external dependencies)
├── Cargo.toml
├── LICENSE
└── .gitignore
```

### Design Principles

- **All `unsafe` in one place.** `win32.rs` contains every FFI declaration. The rest of the codebase calls only safe wrappers.
- **No child processes.** Everything — screen lock, screenshots, taskbar control, registry writes — happens via direct Win32 API calls on Rust-owned threads.
- **Crash-safe.** If the daemon dies while locked, Windows automatically destroys the overlay window and releases the keyboard hook. On next startup, the daemon restores the taskbar and Task Manager.
- **Instant unlock.** The overlay runs on a Rust thread with a Win32 message loop. Posting `WM_QUIT` exits the loop and destroys the window in the same process — no `taskkill`, no delay.

## Security Notes

- The web UI is plain HTTP. Over Tailscale (or Funnel) traffic is encrypted end-to-end. Do **not** expose port 7878 directly to the internet without Funnel.
- The password is hashed with Argon2id. Plaintext is never stored.
- Session tokens are held in memory only and cleared on restart.
- Ctrl+Alt+Del cannot be hooked (it's handled by the Windows kernel). With Task Manager disabled, the options from that screen are limited, but the user can still sign out or shut down from there.

## Troubleshooting

| Problem | Fix |
|---------|-----|
| "Failed to write hosts file" | Run as Administrator |
| Screen lock doesn't hide taskbar | Run as Administrator |
| Screenshot returns an error | Run as Administrator |
| Web UI won't load from phone | Use `tailscale funnel 7878` instead of direct IP |
| Blocked website still loads | Browser DNS cache — restart browser or run `ipconfig /flushdns` |
| Taskbar stuck hidden after crash | Restart the daemon — startup cleanup will restore it |

## License

MIT
