use crate::auth;
use crate::filter;
use crate::locker;
use crate::screenshot;
use crate::state::{Schedule, SharedState};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Json};
use serde::{Deserialize, Serialize};
use tracing::info;

// --------------------------------------------------------------------------
// Helpers
// --------------------------------------------------------------------------

/// Extract bearer token from Authorization header.
fn extract_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

/// Check auth and return 401 if invalid.
async fn require_auth(headers: &HeaderMap, state: &SharedState) -> Result<(), StatusCode> {
    let token = extract_token(headers).ok_or(StatusCode::UNAUTHORIZED)?;
    let st = state.read().await;
    if st.is_authenticated(&token) {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

// --------------------------------------------------------------------------
// Web UI
// --------------------------------------------------------------------------

/// Serve the embedded web UI.
pub async fn serve_ui() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

// --------------------------------------------------------------------------
// Auth
// --------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

pub async fn login(
    State(state): State<SharedState>,
    Json(body): Json<LoginRequest>,
) -> impl IntoResponse {
    let st = state.read().await;

    if st.config.password_hash.is_empty() {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({
            "error": "No password configured. Set one in config.json."
        })));
    }

    if !auth::verify_password(&body.password, &st.config.password_hash) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({
            "error": "Invalid password"
        })));
    }

    drop(st);

    let token = auth::generate_token();
    let mut st = state.write().await;
    st.active_tokens.push(token.clone());
    info!("New auth session created");

    (StatusCode::OK, Json(serde_json::json!({
        "token": token
    })))
}

// --------------------------------------------------------------------------
// Status
// --------------------------------------------------------------------------

#[derive(Serialize)]
pub struct StatusResponse {
    pub app_blocking_active: bool,
    pub web_filtering_active: bool,
    pub screen_locked: bool,
    pub blocked_apps: Vec<String>,
    pub blocked_websites: Vec<String>,
    pub schedules: Vec<Schedule>,
    pub watchdog_timeout_secs: u64,
}

pub async fn get_status(
    headers: HeaderMap,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    if let Err(code) = require_auth(&headers, &state).await {
        return (code, Json(serde_json::json!({"error": "unauthorized"}))).into_response();
    }

    // Bump the heartbeat (write lock, but very brief).
    {
        let mut st = state.write().await;
        st.last_heartbeat = std::time::Instant::now();
    }

    let st = state.read().await;
    let resp = StatusResponse {
        app_blocking_active: st.config.app_blocking_active,
        web_filtering_active: st.config.web_filtering_active,
        screen_locked: st.config.screen_locked,
        blocked_apps: st.config.blocked_apps.clone(),
        blocked_websites: st.config.blocked_websites.clone(),
        schedules: st.config.schedules.clone(),
        watchdog_timeout_secs: st.config.watchdog_timeout_secs,
    };

    (StatusCode::OK, Json(serde_json::json!(resp))).into_response()
}

// --------------------------------------------------------------------------
// App blocking
// --------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct SetAppsRequest {
    pub apps: Vec<String>,
    pub active: bool,
}

pub async fn set_blocked_apps(
    headers: HeaderMap,
    State(state): State<SharedState>,
    Json(body): Json<SetAppsRequest>,
) -> impl IntoResponse {
    if let Err(code) = require_auth(&headers, &state).await {
        return (code, Json(serde_json::json!({"error": "unauthorized"})));
    }

    let mut st = state.write().await;
    st.config.blocked_apps = body.apps;
    st.config.app_blocking_active = body.active;
    let _ = st.save_config();
    info!(
        "App blocklist updated: {} apps, active={}",
        st.config.blocked_apps.len(),
        st.config.app_blocking_active
    );

    (StatusCode::OK, Json(serde_json::json!({"ok": true})))
}

// --------------------------------------------------------------------------
// Web filtering
// --------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct SetWebsitesRequest {
    pub websites: Vec<String>,
    pub active: bool,
}

pub async fn set_blocked_websites(
    headers: HeaderMap,
    State(state): State<SharedState>,
    Json(body): Json<SetWebsitesRequest>,
) -> impl IntoResponse {
    if let Err(code) = require_auth(&headers, &state).await {
        return (code, Json(serde_json::json!({"error": "unauthorized"})));
    }

    let mut st = state.write().await;
    st.config.blocked_websites = body.websites;
    st.config.web_filtering_active = body.active;

    // Apply or clear hosts file entries.
    let result = if body.active {
        filter::apply_blocks(&st.config.blocked_websites)
    } else {
        filter::clear_blocks()
    };

    let _ = st.save_config();

    match result {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e})),
        ),
    }
}

// --------------------------------------------------------------------------
// Screen / input lock
// --------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct LockRequest {
    pub locked: bool,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub template: Option<locker::LockTemplate>,
}

pub async fn set_screen_lock(
    headers: HeaderMap,
    State(state): State<SharedState>,
    Json(body): Json<LockRequest>,
) -> impl IntoResponse {
    if let Err(code) = require_auth(&headers, &state).await {
        return (code, Json(serde_json::json!({"error": "unauthorized"})));
    }

    if body.locked {
        // Locking: run synchronously (taskbar hide + Task Manager disable are fast).
        let text = body.text.unwrap_or_else(|| "This device is locked.".into());
        let template = body.template.unwrap_or_default();
        let result =
            tokio::task::spawn_blocking(move || locker::engage_lock(&text, &template)).await;

        match result {
            Ok(Ok(())) => {
                let mut st = state.write().await;
                st.config.screen_locked = true;
                let _ = st.save_config();
                (StatusCode::OK, Json(serde_json::json!({"ok": true})))
            }
            Ok(Err(e)) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e})),
            ),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Task failed: {e}")})),
            ),
        }
    } else {
        // Unlocking: update state IMMEDIATELY, then run cleanup in background.
        // This makes the UI respond instantly even though the actual kill takes a moment.
        {
            let mut st = state.write().await;
            st.config.screen_locked = false;
            let _ = st.save_config();
        }

        tokio::task::spawn_blocking(|| {
            if let Err(e) = locker::disengage_lock() {
                tracing::error!("Background unlock cleanup failed: {e}");
            }
        });

        (StatusCode::OK, Json(serde_json::json!({"ok": true})))
    }
}

// --------------------------------------------------------------------------
// Schedules
// --------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct SetSchedulesRequest {
    pub schedules: Vec<Schedule>,
}

pub async fn set_schedules(
    headers: HeaderMap,
    State(state): State<SharedState>,
    Json(body): Json<SetSchedulesRequest>,
) -> impl IntoResponse {
    if let Err(code) = require_auth(&headers, &state).await {
        return (code, Json(serde_json::json!({"error": "unauthorized"})));
    }

    let mut st = state.write().await;
    st.config.schedules = body.schedules;
    let _ = st.save_config();
    info!("Schedules updated: {} entries", st.config.schedules.len());

    (StatusCode::OK, Json(serde_json::json!({"ok": true})))
}

// --------------------------------------------------------------------------
// Quick toggle endpoints
// --------------------------------------------------------------------------

pub async fn toggle_app_blocking(
    headers: HeaderMap,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    if let Err(code) = require_auth(&headers, &state).await {
        return (code, Json(serde_json::json!({"error": "unauthorized"})));
    }

    let mut st = state.write().await;
    st.config.app_blocking_active = !st.config.app_blocking_active;
    let _ = st.save_config();
    let active = st.config.app_blocking_active;

    (
        StatusCode::OK,
        Json(serde_json::json!({"app_blocking_active": active})),
    )
}

pub async fn toggle_web_filtering(
    headers: HeaderMap,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    if let Err(code) = require_auth(&headers, &state).await {
        return (code, Json(serde_json::json!({"error": "unauthorized"})));
    }

    let mut st = state.write().await;
    st.config.web_filtering_active = !st.config.web_filtering_active;
    let active = st.config.web_filtering_active;

    let result = if active {
        filter::apply_blocks(&st.config.blocked_websites)
    } else {
        filter::clear_blocks()
    };

    let _ = st.save_config();

    match result {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"web_filtering_active": active})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e})),
        ),
    }
}

// --------------------------------------------------------------------------
// Screenshot
// --------------------------------------------------------------------------

pub async fn take_screenshot(
    headers: HeaderMap,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    if let Err(code) = require_auth(&headers, &state).await {
        return (
            code,
            [("content-type", "application/json")],
            b"{\"error\":\"unauthorized\"}".to_vec(),
        );
    }

    // Run capture in a blocking task since it does synchronous work.
    let result = tokio::task::spawn_blocking(screenshot::capture_screen).await;

    match result {
        Ok(Ok(png_bytes)) => (
            StatusCode::OK,
            [("content-type", "image/png")],
            png_bytes,
        ),
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [("content-type", "application/json")],
            format!("{{\"error\":\"{e}\"}}").into_bytes(),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [("content-type", "application/json")],
            format!("{{\"error\":\"Task failed: {e}\"}}").into_bytes(),
        ),
    }
}

// --------------------------------------------------------------------------
// Watchdog
// --------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct WatchdogRequest {
    pub timeout_secs: u64,
}

pub async fn set_watchdog(
    headers: HeaderMap,
    State(state): State<SharedState>,
    Json(body): Json<WatchdogRequest>,
) -> impl IntoResponse {
    if let Err(code) = require_auth(&headers, &state).await {
        return (code, Json(serde_json::json!({"error": "unauthorized"})));
    }

    let mut st = state.write().await;
    st.config.watchdog_timeout_secs = body.timeout_secs;
    let _ = st.save_config();
    info!("Watchdog timeout set to {}s", body.timeout_secs);

    (StatusCode::OK, Json(serde_json::json!({"ok": true, "timeout_secs": body.timeout_secs})))
}
