//! ITS authentication: one cancellable worker per attempt, with revisioned UI updates.
use crate::auth_storage as storage;
use serde::{Deserialize, Serialize};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};

const PORTAL: &str = "https://portal.its.ac.id/";
const PORTAL_AUTH: &str = "https://portal.its.ac.id/api/auth";
const POLL: Duration = Duration::from_millis(400);
type AuthResult<T> = Result<T, String>;

#[derive(Clone, Copy, Default, PartialEq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Checking,
    SigningIn,
    SignedIn,
    SignedOut,
    Error,
}
impl Phase {
    fn busy(self) -> bool {
        matches!(self, Self::Checking | Self::SigningIn)
    }
}
#[derive(Clone, Default, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub state: Phase,
    pub revision: u64,
    pub name: Option<String>,
    pub username: Option<String>,
    pub message: Option<String>,
}
impl Status {
    fn new(state: Phase) -> Self {
        Self {
            state,
            ..Self::default()
        }
    }
    fn error(message: String) -> Self {
        Self {
            message: Some(message),
            ..Self::new(Phase::Error)
        }
    }
}
#[derive(Default)]
struct Inner {
    status: Option<Status>,
    generation: u64,
    revision: u64,
}
impl Inner {
    fn current(&self, id: u64) -> bool {
        self.generation == id && self.status.as_ref().is_some_and(|s| s.state.busy())
    }
    fn update(&mut self, mut status: Status) -> Status {
        self.revision += 1;
        status.revision = self.revision;
        self.status = Some(status.clone());
        status
    }
    fn begin(&mut self, phase: Phase) -> Option<(u64, Status)> {
        if self.status.as_ref().is_some_and(|s| s.state.busy()) {
            return None;
        }
        self.generation += 1;
        let status = self.update(Status::new(phase));
        Some((self.generation, status))
    }
    fn finish(&mut self, id: u64, status: Status) -> Option<Status> {
        if !self.current(id) {
            return None;
        }
        Some(self.update(status))
    }
}
#[derive(Default)]
pub struct AuthState(Mutex<Inner>);
#[derive(Serialize, Deserialize)]
pub(super) struct Session {
    pub version: u8,
    pub cookies: Vec<(String, String)>,
}
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Restore,
    Interactive,
}
fn label(id: u64) -> String {
    format!("its-auth-{id}")
}
pub(super) fn its_host(host: &str) -> bool {
    host == "its.ac.id" || host.ends_with(".its.ac.id")
}
fn on_portal(url: &tauri::Url) -> bool {
    url.scheme() == "https" && url.host_str() == Some("portal.its.ac.id")
}
fn allowed(url: &tauri::Url) -> bool {
    url.scheme() == "https"
        && url.host_str().is_some_and(|h| {
            its_host(h)
                || matches!(
                    h,
                    "login.microsoftonline.com" | "login.live.com" | "account.live.com"
                )
        })
}
fn authorize(window: &tauri::WebviewWindow) -> AuthResult<()> {
    let url = window.url().map_err(|_| "Untrusted caller")?;
    let local = (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
        || (matches!(url.scheme(), "http" | "https") && url.host_str() == Some("tauri.localhost"))
        || (cfg!(debug_assertions)
            && url.scheme() == "http"
            && matches!(url.host_str(), Some("localhost" | "127.0.0.1"))
            && url.port() == Some(1420));
    if window.label() != "main" || !local {
        return Err("Untrusted caller".into());
    }
    if !cfg!(any(target_os = "windows", target_os = "macos")) {
        return Err("Sign-in is currently supported on Windows and macOS.".into());
    }
    Ok(())
}
fn publish(app: &tauri::AppHandle, status: Status) {
    let _ = app.emit_to("main", "auth:changed", status);
}
fn active(app: &tauri::AppHandle, id: u64) -> bool {
    app.state::<AuthState>().0.lock().unwrap().current(id)
}
fn identity(body: &serde_json::Value) -> AuthResult<Status> {
    let username = body
        .get("preferred_username")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .ok_or("ITS did not return an account identity")?;
    Ok(Status {
        name: body.get("name").and_then(|v| v.as_str()).map(str::to_owned),
        username: Some(username.into()),
        ..Status::new(Phase::SignedIn)
    })
}
fn decode_reply(body: &serde_json::Value) -> AuthResult<Option<Status>> {
    let Some(http) = body.get("http").and_then(|v| v.as_u64()) else {
        return Ok(None);
    };
    match http {
        200 => identity(body).map(Some),
        302 | 401 | 403 => Ok(Some(Status::new(Phase::SignedOut))),
        0 => Err("Could not reach ITS. Check your connection and retry.".into()),
        _ => Err(format!(
            "ITS session verification returned HTTP {http}. Please retry."
        )),
    }
}
// Only the official same-origin API response is read; cookies never enter JavaScript.
fn verify(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    id: u64,
    deadline: Instant,
) -> AuthResult<Status> {
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|_| "Could not start verification")?;
    let key = serde_json::to_string(&format!("__mia_verify_{nonce:x?}")).unwrap();
    window.eval(format!(r#"(() => {{
        if(location.origin!=='https://portal.its.ac.id')return;
        const key={key}, probe={{controller:new AbortController(),result:null}};
        window[key]=probe;
        const timer=setTimeout(()=>probe.controller.abort(),15000);
        fetch('/api/api/sso/get/user-info',{{credentials:'include',redirect:'manual',headers:{{Accept:'application/json'}},signal:probe.controller.signal}})
        .then(async r=>{{const b=r.ok?await r.json():{{}};probe.result={{http:r.type==='opaqueredirect'?302:r.status,name:b.name,preferred_username:b.preferred_username}};}})
        .catch(()=>{{probe.result={{http:0}};}}).finally(()=>clearTimeout(timer));
    }})()"#)).map_err(|_|"Could not check the portal session")?;
    let result = (|| {
        while active(app, id) && Instant::now() < deadline {
            if !window.url().is_ok_and(|u| on_portal(&u)) {
                return Ok(Status::new(Phase::SignedOut));
            }
            let (tx, rx) = std::sync::mpsc::channel();
            window
                .eval_with_callback(format!("window[{key}]?.result ?? null"), move |v| {
                    let _ = tx.send(v);
                })
                .map_err(|_| "Could not check the portal session")?;
            let remaining = deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_secs(2));
            if let Ok(raw) = rx.recv_timeout(remaining)
                && let Ok(body) = serde_json::from_str(&raw)
                && let Some(status) = decode_reply(&body)?
            {
                return Ok(status);
            }
            std::thread::sleep(POLL);
        }
        Err("ITS verification was cancelled or timed out. Please retry.".into())
    })();
    let _ = window.eval(format!(
        "window[{key}]?.controller.abort();delete window[{key}]"
    ));
    result
}
fn capture(window: &tauri::WebviewWindow) -> AuthResult<Session> {
    let cookies = window
        .cookies()
        .map_err(|_| "Could not read the ITS session")?
        .into_iter()
        .filter_map(|mut c| {
            let domain = c.domain()?.to_owned();
            let host = domain.trim_start_matches('.');
            if !its_host(host) {
                return None;
            }
            let origin = format!("https://{host}/");
            if !domain.starts_with('.') {
                c.unset_domain();
            }
            Some((origin, c.to_string()))
        })
        .collect();
    Ok(Session {
        version: 1,
        cookies,
    })
}
fn run(app: &tauri::AppHandle, id: u64, mode: Mode) -> AuthResult<()> {
    let saved = storage::load(app)?;
    if mode == Mode::Restore && saved.is_none() {
        finish(app, id, Ok((Status::new(Phase::SignedOut), None)));
        return Ok(());
    }
    if !active(app, id) {
        return Err("Sign-in cancelled".into());
    }
    let window = tauri::WebviewWindowBuilder::new(
        app,
        label(id),
        tauri::WebviewUrl::External("about:blank".parse().unwrap()),
    )
    .title("ITS account sign-in")
    .inner_size(960.0, 760.0)
    .visible(mode == Mode::Interactive)
    .incognito(true)
    .on_navigation(|u| u.as_str() == "about:blank" || allowed(u))
    .build()
    .map_err(|_| "Could not open ITS sign-in")?;
    let close_app = app.clone();
    window.on_window_event(move |e| {
        if matches!(e, tauri::WindowEvent::Destroyed) {
            let app = close_app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let state = app.state::<AuthState>();
                let mut inner = state.0.lock().unwrap();
                if let Some(status) = inner.finish(id, Status::new(Phase::SignedOut)) {
                    drop(inner);
                    publish(&app, status);
                }
            });
        }
    });
    let result = (|| {
        if !active(app, id) {
            return Err("Sign-in cancelled".into());
        }
        if let Some(session) = saved {
            storage::restore_cookies(&window, &session)?;
        }
        if !active(app, id) {
            return Err("Sign-in cancelled".into());
        }
        let start = if mode == Mode::Interactive {
            PORTAL_AUTH
        } else {
            PORTAL
        };
        window
            .navigate(start.parse().unwrap())
            .map_err(|_| "Could not reach ITS")?;
        let deadline =
            Instant::now() + Duration::from_secs(if mode == Mode::Interactive { 900 } else { 40 });
        let mut handoff = false;
        while active(app, id) && Instant::now() < deadline {
            let Ok(url) = window.url() else {
                return Ok((Status::new(Phase::SignedOut), None));
            };
            if on_portal(&url) {
                let attempt = verify(
                    app,
                    &window,
                    id,
                    deadline.min(Instant::now() + Duration::from_secs(18)),
                );
                match attempt {
                    Ok(status) if status.state == Phase::SignedIn => {
                        return Ok((status, Some(capture(&window)?)));
                    }
                    Ok(_) if mode == Mode::Restore && !handoff => {
                        handoff = true;
                        window
                            .navigate(PORTAL_AUTH.parse().unwrap())
                            .map_err(|_| "Could not restore ITS sign-in")?;
                    }
                    Ok(status) if mode == Mode::Restore => return Ok((status, None)),
                    Err(error) => {
                        let state = app.state::<AuthState>();
                        let mut inner = state.0.lock().unwrap();
                        if inner.current(id) {
                            let mut status = inner.status.clone().unwrap();
                            status.message = Some(error);
                            let status = inner.update(status);
                            drop(inner);
                            publish(app, status);
                        }
                    }
                    _ => {} // MFA or portal bootstrap is still in progress. Never erase its cookies.
                }
            } else if mode == Mode::Restore
                && url.as_str() != "about:blank"
                && !url.host_str().is_some_and(its_host)
            {
                // Restoring must not leave an invisible password/MFA prompt open.
                return Ok((Status::new(Phase::SignedOut), None));
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        Err("Sign-in was cancelled or timed out. Please try again.".into())
    })();
    // Complete the state transition before closing; Destroyed must not cancel success.
    finish(app, id, result);
    let _ = window.close();
    Ok(())
}
fn finish(app: &tauri::AppHandle, id: u64, result: AuthResult<(Status, Option<Session>)>) {
    let state = app.state::<AuthState>();
    let mut inner = state.0.lock().unwrap();
    if !inner.current(id) {
        return;
    }
    let status = match result {
        Ok((status, Some(session))) => match storage::save(app, &session) {
            Ok(()) => status,
            Err(e) => Status::error(e),
        },
        Ok((status, None)) => status,
        Err(e) => Status::error(e),
    };
    let signed_in = status.state == Phase::SignedIn;
    let status = inner.finish(id, status).unwrap();
    drop(inner);
    publish(app, status);
    if signed_in && let Some(main) = app.get_webview_window("main") {
        let _ = main.set_focus();
    }
}
fn launch(app: tauri::AppHandle, id: u64, mode: Mode) {
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = run(&app, id, mode) {
            finish(&app, id, Err(error));
        }
    });
}
#[tauri::command]
pub async fn auth_get_status(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> AuthResult<Status> {
    authorize(&window)?;
    let state = app.state::<AuthState>();
    let mut inner = state.0.lock().unwrap();
    if let Some(status) = &inner.status {
        return Ok(status.clone());
    }
    let (id, status) = inner.begin(Phase::Checking).unwrap();
    drop(inner);
    publish(&app, status.clone());
    launch(app, id, Mode::Restore);
    Ok(status)
}
#[tauri::command]
pub async fn auth_start_login(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> AuthResult<Status> {
    authorize(&window)?;
    let state = app.state::<AuthState>();
    let mut inner = state.0.lock().unwrap();
    let Some((id, status)) = inner.begin(Phase::SigningIn) else {
        let status = inner.status.clone().unwrap();
        let id = inner.generation;
        drop(inner);
        if status.state == Phase::SigningIn
            && let Some(w) = app.get_webview_window(&label(id))
        {
            let _ = w.set_focus();
        }
        return Ok(status);
    };
    drop(inner);
    publish(&app, status.clone());
    launch(app, id, Mode::Interactive);
    Ok(status)
}
async fn reset(app: tauri::AppHandle, forget: bool) -> AuthResult<Status> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AuthState>();
        let mut inner = state.0.lock().unwrap();
        if !forget && !inner.status.as_ref().is_some_and(|s| s.state.busy()) {
            return Ok(inner
                .status
                .clone()
                .unwrap_or_else(|| Status::new(Phase::SignedOut)));
        }
        // Logout is serialized with persistence. Late worker completions cannot resurrect it.
        if forget {
            storage::clear(&app)?;
        }
        let old_id = inner.generation;
        inner.generation += 1;
        let status = inner.update(Status::new(Phase::SignedOut));
        drop(inner);
        publish(&app, status.clone());
        if let Some(w) = app.get_webview_window(&label(old_id)) {
            let _ = w.close();
        }
        Ok(status)
    })
    .await
    .map_err(|_| "Session task failed".to_string())?
}
#[tauri::command]
pub async fn auth_cancel_login(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> AuthResult<Status> {
    authorize(&window)?;
    reset(app, false).await
}
#[tauri::command]
pub async fn auth_logout(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> AuthResult<Status> {
    authorize(&window)?;
    reset(app, true).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_approved_https_origins_are_allowed() {
        assert!(allowed(&"https://my.its.ac.id/authorize".parse().unwrap()));
        assert!(allowed(
            &"https://login.microsoftonline.com/common".parse().unwrap()
        ));
        for url in [
            "http://portal.its.ac.id/",
            "https://its.ac.id.evil.test/",
            "https://notits.ac.id/",
        ] {
            assert!(!allowed(&url.parse().unwrap()));
        }
    }
    #[test]
    fn concurrent_attempts_share_one_worker() {
        let mut state = Inner::default();
        let (id, status) = state.begin(Phase::Checking).unwrap();
        assert!(state.begin(Phase::Checking).is_none());
        assert!(state.begin(Phase::SigningIn).is_none());
        assert!(state.current(id));
        assert_eq!(status.revision, 1);
    }
    #[test]
    fn cancellation_invalidates_old_results_without_overwriting_a_new_attempt() {
        let mut state = Inner::default();
        let (old, _) = state.begin(Phase::SigningIn).unwrap();
        state.generation += 1;
        state.update(Status::new(Phase::SignedOut));
        let (new, _) = state.begin(Phase::SigningIn).unwrap();
        assert!(state.finish(old, Status::new(Phase::SignedIn)).is_none());
        assert!(state.current(new));
        assert!(state.finish(new, Status::new(Phase::SignedIn)).is_some());
        assert!(!state.current(new));
    }
    #[test]
    fn revision_orders_event_and_command_results() {
        let mut state = Inner::default();
        let (id, initial) = state.begin(Phase::Checking).unwrap();
        let done = state.finish(id, Status::new(Phase::SignedIn)).unwrap();
        assert!(done.revision > initial.revision);
    }
    #[test]
    fn verification_does_not_accept_redirects_or_missing_identity() {
        for http in [302, 401, 403] {
            assert_eq!(
                decode_reply(&serde_json::json!({"http":http}))
                    .unwrap()
                    .unwrap()
                    .state,
                Phase::SignedOut
            );
        }
        assert!(decode_reply(&serde_json::json!({"http":200})).is_err());
        assert!(decode_reply(&serde_json::json!({"http":200,"preferred_username":" "})).is_err());
        assert!(decode_reply(&serde_json::json!({"http":503})).is_err());
        assert!(decode_reply(&serde_json::Value::Null).unwrap().is_none());
        assert_eq!(
            decode_reply(
                &serde_json::json!({"http":200,"preferred_username":"test-student","name":"Test"})
            )
            .unwrap()
            .unwrap()
            .state,
            Phase::SignedIn
        );
    }
}
