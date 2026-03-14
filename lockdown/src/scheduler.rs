use crate::state::{ScheduleAction, SharedState, Weekday};
use chrono::{Datelike, Local};
use std::time::Duration;
use tracing::{debug, info};

/// Runs the scheduling loop. Checks every `interval` whether any schedule
/// rules should activate or deactivate features.
///
/// This function runs forever and should be spawned as a background task.
pub async fn run_scheduler(state: SharedState, interval: Duration) {
    loop {
        tokio::time::sleep(interval).await;

        let now = Local::now();
        let current_time = now.time();
        let current_day = Weekday::from_chrono(now.weekday());

        let schedules;
        {
            let st = state.read().await;
            schedules = st.config.schedules.clone();
        }

        let mut should_block_apps = false;
        let mut should_block_web = false;
        let mut should_lock_screen = false;

        for schedule in &schedules {
            if !schedule.enabled {
                continue;
            }

            if !schedule.days.contains(&current_day) {
                continue;
            }

            let is_active = if schedule.start_time <= schedule.end_time {
                // Normal range, e.g. 09:00 - 17:00
                current_time >= schedule.start_time && current_time < schedule.end_time
            } else {
                // Overnight range, e.g. 22:00 - 06:00
                current_time >= schedule.start_time || current_time < schedule.end_time
            };

            if is_active {
                debug!(
                    "Schedule '{}' is active ({} - {})",
                    schedule.name, schedule.start_time, schedule.end_time
                );
                match &schedule.action {
                    ScheduleAction::BlockApps => should_block_apps = true,
                    ScheduleAction::BlockWeb => should_block_web = true,
                    ScheduleAction::LockScreen => should_lock_screen = true,
                    ScheduleAction::BlockAll => {
                        should_block_apps = true;
                        should_block_web = true;
                        should_lock_screen = true;
                    }
                }
            }
        }

        // Apply schedule-driven state changes. We only *activate* features
        // from schedules — manual overrides from the API take priority, so
        // we never deactivate something the controller turned on manually.
        {
            let mut st = state.write().await;

            if should_block_apps && !st.config.app_blocking_active {
                info!("Schedule activating app blocking");
                st.config.app_blocking_active = true;
            }

            if should_block_web && !st.config.web_filtering_active {
                info!("Schedule activating web filtering");
                st.config.web_filtering_active = true;
                let domains = st.config.blocked_websites.clone();
                // Apply hosts file changes outside the lock would be better,
                // but for simplicity we do it here. The write is fast.
                if let Err(e) = crate::filter::apply_blocks(&domains) {
                    tracing::error!("Schedule failed to apply web blocks: {e}");
                }
            }

            if should_lock_screen && !st.config.screen_locked {
                info!("Schedule activating screen lock");
                st.config.screen_locked = true;
                if let Err(e) = crate::locker::engage_lock(
                    "This device is locked by schedule.",
                    &crate::locker::LockTemplate::default(),
                ) {
                    tracing::error!("Schedule failed to engage lock: {e}");
                }
            }
        }
    }
}
