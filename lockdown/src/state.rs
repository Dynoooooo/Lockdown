use chrono::NaiveTime;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Days of the week for scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    pub fn from_chrono(w: chrono::Weekday) -> Self {
        match w {
            chrono::Weekday::Mon => Self::Monday,
            chrono::Weekday::Tue => Self::Tuesday,
            chrono::Weekday::Wed => Self::Wednesday,
            chrono::Weekday::Thu => Self::Thursday,
            chrono::Weekday::Fri => Self::Friday,
            chrono::Weekday::Sat => Self::Saturday,
            chrono::Weekday::Sun => Self::Sunday,
        }
    }
}

/// What a schedule entry does when active.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleAction {
    BlockApps,
    BlockWeb,
    LockScreen,
    BlockAll,
}

/// A time-based rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    pub id: String,
    pub name: String,
    pub days: Vec<Weekday>,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    pub action: ScheduleAction,
    pub enabled: bool,
}

/// The full application configuration / live state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Argon2 hash of the controller password.
    pub password_hash: String,
    /// Process names to block (e.g. "notepad.exe", "discord.exe").
    pub blocked_apps: Vec<String>,
    /// Domains to block via hosts file (e.g. "reddit.com").
    pub blocked_websites: Vec<String>,
    /// Time-based schedules.
    pub schedules: Vec<Schedule>,
    /// Whether the screen is currently locked.
    pub screen_locked: bool,
    /// Whether app blocking is currently active.
    pub app_blocking_active: bool,
    /// Whether web filtering is currently active.
    pub web_filtering_active: bool,
    /// Port the web server listens on.
    pub listen_port: u16,
    /// Watchdog timeout in seconds. If no authenticated request is received
    /// within this period while locked, auto-unlock. 0 = disabled.
    #[serde(default)]
    pub watchdog_timeout_secs: u64,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            password_hash: String::new(),
            blocked_apps: Vec::new(),
            blocked_websites: Vec::new(),
            schedules: Vec::new(),
            screen_locked: false,
            app_blocking_active: false,
            web_filtering_active: false,
            listen_port: 7878,
            watchdog_timeout_secs: 0,
        }
    }
}

use std::time::Instant;

/// Thread-safe shared state.
pub type SharedState = Arc<RwLock<AppState>>;

#[derive(Debug)]
pub struct AppState {
    pub config: AppConfig,
    /// Currently valid auth tokens (simple session tokens).
    pub active_tokens: Vec<String>,
    /// Path to the config file on disk.
    pub config_path: String,
    /// Last time an authenticated request was received (for watchdog).
    pub last_heartbeat: Instant,
}

impl AppState {
    pub fn new(config: AppConfig, config_path: String) -> Self {
        Self {
            config,
            active_tokens: Vec::new(),
            config_path,
            last_heartbeat: Instant::now(),
        }
    }

    /// Persist config to disk. Returns an error message on failure.
    pub fn save_config(&self) -> Result<(), String> {
        let json = serde_json::to_string_pretty(&self.config)
            .map_err(|e| format!("Failed to serialize config: {e}"))?;
        std::fs::write(&self.config_path, json)
            .map_err(|e| format!("Failed to write config to {}: {e}", self.config_path))?;
        Ok(())
    }

    /// Check if a token is valid.
    pub fn is_authenticated(&self, token: &str) -> bool {
        self.active_tokens.iter().any(|t| t == token)
    }
}
