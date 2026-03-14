//! Watchdog: if the phone stops pinging while the screen is locked,
//! auto-unlock after the configured timeout.

use crate::state::SharedState;
use std::time::Duration;
use tracing::{info, warn};

/// Runs the watchdog loop. Checks every `interval` whether the heartbeat
/// has expired while the screen is locked.
///
/// This function runs forever and should be spawned as a background task.
pub async fn run_watchdog(state: SharedState, interval: Duration) {
    loop {
        tokio::time::sleep(interval).await;

        let (is_locked, timeout_secs, elapsed_secs) = {
            let st = state.read().await;
            (
                st.config.screen_locked,
                st.config.watchdog_timeout_secs,
                st.last_heartbeat.elapsed().as_secs(),
            )
        };

        // Skip if watchdog is disabled (timeout == 0) or screen isn't locked.
        if timeout_secs == 0 || !is_locked {
            continue;
        }

        if elapsed_secs >= timeout_secs {
            warn!(
                "Watchdog triggered: no heartbeat for {}s (timeout {}s). Auto-unlocking.",
                elapsed_secs, timeout_secs
            );

            // Update state first.
            {
                let mut st = state.write().await;
                st.config.screen_locked = false;
                let _ = st.save_config();
            }

            // Disengage lock in a blocking task.
            tokio::task::spawn_blocking(|| {
                if let Err(e) = crate::locker::disengage_lock() {
                    tracing::error!("Watchdog auto-unlock failed: {e}");
                }
            });

            info!("Watchdog auto-unlock complete");
        }
    }
}
