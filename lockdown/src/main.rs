mod api;
mod auth;
mod blocker;
mod filter;
mod locker;
mod scheduler;
mod screenshot;
mod state;

use axum::routing::{get, post};
use axum::Router;
use state::{AppConfig, AppState, SharedState};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info};

const DEFAULT_CONFIG_PATH: &str = "lockdown_config.json";

fn load_or_create_config(path: &str) -> AppConfig {
    if Path::new(path).exists() {
        match std::fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str::<AppConfig>(&content) {
                Ok(config) => {
                    info!("Loaded config from {path}");
                    return config;
                }
                Err(e) => {
                    error!("Failed to parse {path}: {e}");
                    error!("Starting with default config. Fix or delete the file.");
                }
            },
            Err(e) => {
                error!("Failed to read {path}: {e}");
            }
        }
    }

    // Create default config with a password prompt.
    let config = AppConfig::default();
    info!("No config found — creating {path} with defaults.");
    info!("IMPORTANT: You must set a password before the app is usable.");
    config
}

/// Interactive first-run setup: ask for a password if none is set.
fn ensure_password(config: &mut AppConfig) {
    if !config.password_hash.is_empty() {
        return;
    }

    eprintln!();
    eprintln!("╔══════════════════════════════════════════╗");
    eprintln!("║  LOCKDOWN — First Run Setup              ║");
    eprintln!("╠══════════════════════════════════════════╣");
    eprintln!("║  No password configured.                 ║");
    eprintln!("║  Enter a password for the remote UI.     ║");
    eprintln!("╚══════════════════════════════════════════╝");
    eprintln!();

    loop {
        eprint!("  Password: ");
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_err() {
            eprintln!("  Failed to read input. Try again.");
            continue;
        }
        let password = input.trim();
        if password.len() < 4 {
            eprintln!("  Password too short (min 4 characters). Try again.");
            continue;
        }

        match auth::hash_password(password) {
            Ok(hash) => {
                config.password_hash = hash;
                eprintln!("  Password set successfully.");
                eprintln!();
                return;
            }
            Err(e) => {
                eprintln!("  Failed to hash password: {e}");
                continue;
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Initialize logging.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "lockdown=info".into()),
        )
        .init();

    // Load config.
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_CONFIG_PATH.to_string());

    let mut config = load_or_create_config(&config_path);
    ensure_password(&mut config);

    let port = config.listen_port;

    // Build shared state.
    let app_state: SharedState = Arc::new(RwLock::new(AppState::new(config, config_path.clone())));

    // Save initial config (in case we just set a password).
    {
        let st = app_state.read().await;
        if let Err(e) = st.save_config() {
            error!("Failed to save initial config: {e}");
        }
    }

    // Spawn background tasks.
    let blocker_state = Arc::clone(&app_state);
    tokio::spawn(async move {
        blocker::run_blocker(blocker_state, Duration::from_secs(3)).await;
    });

    let scheduler_state = Arc::clone(&app_state);
    tokio::spawn(async move {
        scheduler::run_scheduler(scheduler_state, Duration::from_secs(30)).await;
    });

    // Build router.
    let app = Router::new()
        // Web UI
        .route("/", get(api::serve_ui))
        // Auth
        .route("/api/login", post(api::login))
        // Status
        .route("/api/status", get(api::get_status))
        // App blocking
        .route("/api/apps", post(api::set_blocked_apps))
        .route("/api/apps/toggle", post(api::toggle_app_blocking))
        // Web filtering
        .route("/api/websites", post(api::set_blocked_websites))
        .route("/api/websites/toggle", post(api::toggle_web_filtering))
        // Screen lock
        .route("/api/lock", post(api::set_screen_lock))
        // Schedules
        .route("/api/schedules", post(api::set_schedules))
        // Screenshot
        .route("/api/screenshot", get(api::take_screenshot))
        .with_state(app_state);

    // Start server.
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Lockdown listening on http://{addr}");
    info!("Access the control panel from your phone at http://<tailscale-ip>:{port}");

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            error!("Failed to bind to {addr}: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = axum::serve(listener, app).await {
        error!("Server error: {e}");
        std::process::exit(1);
    }
}
