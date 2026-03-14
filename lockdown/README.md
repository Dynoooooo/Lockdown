# Lockdown

Remote device control daemon for Windows. Runs on a PC and exposes a mobile-first web UI you control from your phone over Tailscale.

## Features

- **App Blocking** — Kill processes by name (e.g. `discord.exe`, `chrome.exe`). Polls every 3 seconds.
- **Web Filtering** — Block domains via the Windows hosts file. Automatically blocks `www.` variants. Includes one-tap templates for common categories (social media, pornography, video streaming, gaming, news, shopping, gambling, dating, Reddit/forums).
- **Screen/Input Lock** — Lock the workstation and block all keyboard/mouse input via Windows APIs.
- **Screenshots** — Capture the PC screen on demand or auto-capture every 5 seconds. Tap to view fullscreen.
- **Schedules** — Time-based rules that activate features on specific days and time ranges. Supports overnight ranges (e.g. 22:00–06:00).
- **Remote Web UI** — Mobile-first control panel. No external dependencies — the entire UI is embedded in the binary.
- **Password Auth** — Argon2id-hashed password with session tokens.

## Requirements

- **Windows 10/11** (the target machine)
- **Rust toolchain** — install from https://rustup.rs
- **Administrator privileges** — required for hosts file editing and input blocking
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

For all features to work (hosts file editing, input blocking, screenshots), run from an elevated prompt:

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
│   ├── main.rs        — Entry point, config loading, server setup
│   ├── state.rs       — Shared state, config structs, persistence
│   ├── auth.rs        — Argon2 password hashing, session tokens
│   ├── api.rs         — HTTP route handlers (axum)
│   ├── blocker.rs     — Process monitoring and termination loop
│   ├── filter.rs      — Hosts file management for domain blocking
│   ├── locker.rs      — Windows API FFI for screen/input lock
│   ├── screenshot.rs  — Screen capture via PowerShell/.NET
│   └── scheduler.rs   — Time-based rule evaluation loop
├── static/
│   └── index.html     — Embedded mobile-first web UI (no external dependencies)
├── Cargo.toml
└── .gitignore
```

## Security Notes

- The web UI is plain HTTP. Over Tailscale (or Funnel) traffic is encrypted end-to-end. Do **not** expose port 7878 directly to the internet without Funnel.
- The password is hashed with Argon2id. Plaintext is never stored.
- Session tokens are held in memory only and cleared on restart.
- The app needs Administrator for hosts file editing, input blocking, and screenshots. Process killing works without admin but may fail on elevated processes.

## Troubleshooting

| Problem | Fix |
|---------|-----|
| "Failed to write hosts file" | Run as Administrator |
| "BlockInput failed" | Run as Administrator |
| "LockWorkStation failed" | App may be running in session 0 — run interactively instead |
| Screenshot fails | Run as Administrator; ensure PowerShell is available |
| Web UI won't load from phone | Use `tailscale funnel 7878` instead of direct IP |
| Blocked website still loads | Browser DNS cache — restart browser or run `ipconfig /flushdns` |

## License

MIT
