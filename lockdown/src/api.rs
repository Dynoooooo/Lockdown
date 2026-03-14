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
}

pub async fn get_status(
    headers: HeaderMap,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    if let Err(code) = require_auth(&headers, &state).await {
        return (code, Json(serde_json::json!({"error": "unauthorized"}))).into_response();
    }

    let st = state.read().await;
    let resp = StatusResponse {
        app_blocking_active: st.config.app_blocking_active,
        web_filtering_active: st.config.web_filtering_active,
        screen_locked: st.config.screen_locked,
        blocked_apps: st.config.blocked_apps.clone(),
        blocked_websites: st.config.blocked_websites.clone(),
        schedules: st.config.schedules.clone(),
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
}

pub async fn set_screen_lock(
    headers: HeaderMap,
    State(state): State<SharedState>,
    Json(body): Json<LockRequest>,
) -> impl IntoResponse {
    if let Err(code) = require_auth(&headers, &state).await {
        return (code, Json(serde_json::json!({"error": "unauthorized"})));
    }

    let result = if body.locked {
        locker::engage_full_lock()
    } else {
        locker::disengage_lock()
    };

    match result {
        Ok(()) => {
            let mut st = state.write().await;
            st.config.screen_locked = body.locked;
            let _ = st.save_config();
            (StatusCode::OK, Json(serde_json::json!({"ok": true})))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e})),
        ),
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
