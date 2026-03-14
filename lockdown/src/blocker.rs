use crate::state::SharedState;
use std::time::Duration;
use sysinfo::System;
use tracing::{debug, info, warn};

/// Runs the app-blocking loop. Checks for blocked processes every `interval`
/// and terminates any that match the blocklist.
///
/// This function runs forever and should be spawned as a background task.
pub async fn run_blocker(state: SharedState, interval: Duration) {
    let mut sys = System::new();

    loop {
        tokio::time::sleep(interval).await;

        let blocked_apps: Vec<String>;
        let active: bool;

        // Hold the read lock only long enough to clone what we need.
        {
            let st = state.read().await;
            active = st.config.app_blocking_active;
            blocked_apps = st.config.blocked_apps.clone();
        }

        if !active || blocked_apps.is_empty() {
            continue;
        }

        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        for (pid, process) in sys.processes() {
            let name = process.name().to_string_lossy().to_lowercase();
            let should_block = blocked_apps
                .iter()
                .any(|app| name == app.to_lowercase());

            if should_block {
                info!("Terminating blocked process: {} (pid {})", name, pid);
                if !process.kill() {
                    warn!("Failed to terminate process {} (pid {})", name, pid);
                } else {
                    debug!("Successfully terminated {} (pid {})", name, pid);
                }
            }
        }
    }
}
