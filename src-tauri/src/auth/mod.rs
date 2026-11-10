//! ITS sign-in and connection state.
mod apps;
mod logout;
mod storage;

use self::apps::{App, AppStatus, Connection, ConnectionState};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};
use tokio_util::sync::CancellationToken;
use zeroize::Zeroize;

const PORTAL_AUTH: &str = "https://portal.its.ac.id/api/auth";
const POLL: Duration = Duration::from_millis(400);
pub(super) type AuthResult<T> = Result<T, String>;

#[derive(Clone, Copy, Default, PartialEq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Checking,
    SigningIn,
    SigningOut,
    SignedIn,
    SignedOut,
    Error,
}
impl Phase {
    fn busy(self) -> bool {
        matches!(self, Self::Checking | Self::SigningIn | Self::SigningOut)
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
    pub apps: Vec<AppStatus>,
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
    clients: HashMap<App, Connection>,
    session: Option<Arc<Session>>,
    cancel: CancellationToken,
}
impl Inner {
    fn clear_clients(&mut self) {
        self.clients.clear();
    }
    fn cancel_work(&mut self) {
        self.cancel.cancel();
        self.cancel = CancellationToken::new();
        self.generation += 1;
        self.clear_clients();
    }
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
        self.cancel_work();
        let status = self.update(Status::new(phase));
        Some((self.generation, status))
    }
    fn finish(&mut self, id: u64, status: Status) -> Option<Status> {
        if !self.current(id) {
            return None;
        }
        Some(self.update(status))
    }
    fn connecting(&self, id: u64) -> bool {
        self.generation == id
            && self
                .status
                .as_ref()
                .is_some_and(|status| status.state == Phase::SignedIn)
    }
    fn remember(&mut self, id: u64, session: &Session) -> bool {
        if !self.current(id) {
            return false;
        }
        self.session = Some(Arc::new(session.clone()));
        true
    }
}
#[derive(Default)]
pub struct AuthState(Mutex<Inner>);
impl AuthState {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|poisoned| {
            let mut inner = poisoned.into_inner();
            inner.cancel_work();
            inner.session = None;
            inner.update(Status::error(
                "Authentication was interrupted. Please sign in again.".into(),
            ));
            self.0.clear_poison();
            inner
        })
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub(super) struct Session {
    pub version: u8,
    pub cookies: Vec<(String, String)>,
}
impl Drop for Session {
    fn drop(&mut self) {
        for (_, cookie) in &mut self.cookies {
            cookie.zeroize();
        }
    }
}
impl Session {
    // Keep only the host-only central SSO cookie.
    pub(super) fn central_only(&self) -> AuthResult<Self> {
        if self.version != 1 || self.cookies.len() > 128 {
            return Err("Unsupported saved session. Please sign in again.".into());
        }
        let cookies = self
            .cookies
            .iter()
            .find_map(|(origin, raw)| {
                let url = tauri::Url::parse(origin).ok()?;
                if raw.len() > 8192
                    || !url.username().is_empty()
                    || url.password().is_some()
                    || url.fragment().is_some()
                    || url.query().is_some()
                    || url.path() != "/"
                    || url.scheme() != "https"
                    || url.host_str() != Some("my.its.ac.id")
                    || url.port_or_known_default() != Some(443)
                {
                    return None;
                }
                let mut cookie = tauri::webview::Cookie::parse(raw.to_owned()).ok()?;
                if cookie.name() != "session"
                    || cookie.value().is_empty()
                    || cookie.value().bytes().any(|byte| byte <= 32 || byte >= 127)
                    || cookie
                        .domain()
                        .is_some_and(|domain| domain.trim_start_matches('.') != "my.its.ac.id")
                    || cookie.path().is_some_and(|path| path != "/")
                    || cookie.max_age().is_some_and(|age| age.whole_seconds() <= 0)
                    || cookie.expires_datetime().is_some_and(|expiry| {
                        expiry.unix_timestamp()
                            <= std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs() as i64
                    })
                {
                    return None;
                }
                cookie.unset_domain();
                cookie.set_path("/");
                cookie.set_secure(true);
                cookie.set_http_only(true);
                Some(("https://my.its.ac.id/".into(), cookie.to_string()))
            })
            .ok_or("No central ITS session is saved. Please sign in again.")?;
        Ok(Self {
            version: 1,
            cookies: vec![cookies],
        })
    }
}
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Restore,
    Interactive,
}
fn label(id: u64) -> String {
    format!("its-auth-{id}")
}
fn on_portal(url: &tauri::Url) -> bool {
    url.scheme() == "https"
        && url.host_str() == Some("portal.its.ac.id")
        && url.port_or_known_default() == Some(443)
}
pub(super) fn allowed(url: &tauri::Url) -> bool {
    url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(
            url.host_str(),
            Some(
                "portal.its.ac.id"
                    | "my.its.ac.id"
                    | "login.microsoftonline.com"
                    | "login.live.com"
                    | "account.live.com"
            )
        )
}
fn local_origin(url: &tauri::Url) -> bool {
    if !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    (url.scheme() == "tauri" && url.host_str() == Some("localhost") && url.port().is_none())
        || (matches!(url.scheme(), "http" | "https")
            && url.host_str() == Some("tauri.localhost")
            && url.port().is_none())
        || (cfg!(debug_assertions)
            && url.scheme() == "http"
            && matches!(url.host_str(), Some("localhost" | "127.0.0.1"))
            && url.port() == Some(1420))
}
fn authorize(window: &tauri::WebviewWindow) -> AuthResult<()> {
    let url = window.url().map_err(|_| "Untrusted caller")?;
    if window.label() != "main" || !local_origin(&url) {
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
    app.state::<AuthState>().lock().current(id)
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
            "User session verification returned HTTP {http}. Please retry."
        )),
    }
}
// Return account identity, never cookies, through the webview.
pub(super) fn verify(
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
        fetch('/api/api/sso/get/user-info',{{credentials:'include',redirect:'manual',referrer:'https://portal.its.ac.id/',referrerPolicy:'origin',headers:{{Accept:'application/json'}},signal:probe.controller.signal}})
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
        .cookies_for_url("https://my.its.ac.id/".parse().unwrap())
        .map_err(|_| "Could not read the User session")?
        .into_iter()
        .filter_map(|mut c| {
            let domain = c.domain()?.to_owned();
            let host = domain.trim_start_matches('.');
            if host != "my.its.ac.id" || c.name() != "session" {
                return None;
            }
            let origin = format!("https://{host}/");
            if !domain.starts_with('.') {
                c.unset_domain();
            }
            Some((origin, c.to_string()))
        })
        .collect();
    Session {
        version: 1,
        cookies,
    }
    .central_only()
}
fn probe_delay(failures: u32) -> Duration {
    Duration::from_secs(2u64.saturating_pow(failures).min(30))
}
fn run(app: &tauri::AppHandle, id: u64) -> AuthResult<()> {
    let saved = app.state::<AuthState>().lock().session.clone();
    if !active(app, id) {
        return Err("Sign-in cancelled".into());
    }
    #[cfg(target_os = "windows")]
    let profile = tempfile::Builder::new()
        .prefix("miaplusplus-signin-")
        .tempdir()
        .map_err(|_| "Could not create a private sign-in profile.")?;
    let builder = tauri::WebviewWindowBuilder::new(
        app,
        label(id),
        tauri::WebviewUrl::External("about:blank".parse().unwrap()),
    )
    .title("ITS account sign-in")
    .inner_size(960.0, 760.0)
    .visible(true)
    .incognito(true)
    .on_navigation(|url| url.as_str() == "about:blank" || allowed(url));
    #[cfg(target_os = "windows")]
    let builder = builder.data_directory(profile.path().to_owned());
    let window = builder.build().map_err(|_| "Could not open ITS sign-in")?;
    let close_app = app.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) {
            let app = close_app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let state = app.state::<AuthState>();
                let mut inner = state.lock();
                if let Some(status) = inner.finish(id, Status::new(Phase::SignedOut)) {
                    inner.cancel.cancel();
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
        window
            .navigate(PORTAL_AUTH.parse().unwrap())
            .map_err(|_| "Could not reach ITS")?;
        let deadline = Instant::now() + Duration::from_secs(900);
        let mut failures = 0u32;
        while active(app, id) && Instant::now() < deadline {
            let url = window.url().map_err(|_| "The sign-in window closed.")?;
            if on_portal(&url) && !url.path().starts_with("/api/") {
                match verify(
                    app,
                    &window,
                    id,
                    deadline.min(Instant::now() + Duration::from_secs(18)),
                ) {
                    Ok(status) if status.state == Phase::SignedIn => {
                        return Ok((status, Some(capture(&window)?)));
                    }
                    Err(error) => {
                        let state = app.state::<AuthState>();
                        let mut inner = state.lock();
                        if inner.current(id) {
                            let mut status = inner.status.clone().unwrap();
                            status.message = Some(error);
                            let status = inner.update(status);
                            drop(inner);
                            publish(app, status);
                        }
                    }
                    _ => {}
                }
                failures = failures.saturating_add(1);
            }
            let until = deadline.min(Instant::now() + probe_delay(failures));
            while active(app, id) && Instant::now() < until {
                std::thread::sleep(POLL);
            }
        }
        Err("Sign-in was cancelled or timed out. Please try again.".into())
    })();
    // Publish success before Destroyed can cancel it.
    finish(app, id, result, true);
    let _ = window.close();
    Ok(())
}
fn finish(
    app: &tauri::AppHandle,
    id: u64,
    result: AuthResult<(Status, Option<Session>)>,
    persist: bool,
) {
    if !active(app, id) {
        return;
    }
    let prepared = if persist {
        match &result {
            Ok((_, Some(session))) => Some(storage::prepare(session)),
            _ => None,
        }
    } else {
        None
    };
    let state = app.state::<AuthState>();
    let mut inner = state.lock();
    if !inner.current(id) {
        return;
    }
    let mut status = match result {
        Ok((status, Some(session))) => {
            let committed = match prepared {
                Some(result) => result.and_then(|prepared| prepared.commit()),
                None => Ok(()),
            };
            match committed {
                Ok(()) => {
                    inner.session = Some(Arc::new(session));
                    status
                }
                Err(error) => Status::error(error),
            }
        }
        Ok((status, None)) => status,
        Err(error) => Status::error(error),
    };
    let signed_in = status.state == Phase::SignedIn;
    if status.state == Phase::SignedOut {
        inner.session = None;
    }
    if signed_in {
        status.apps = apps::checking();
    }
    let status = inner.finish(id, status).unwrap();
    drop(inner);
    publish(app, status);
    if signed_in {
        launch_apps(app.clone(), id);
        if let Some(main) = app.get_webview_window("main") {
            let _ = main.set_focus();
        }
    }
}

fn launch_apps(app: tauri::AppHandle, id: u64) {
    let (session, username, cancel, targets) = {
        let state = app.state::<AuthState>();
        let inner = state.lock();
        if !inner.connecting(id) {
            return;
        }
        let Some(session) = inner.session.clone() else {
            return;
        };
        let status = inner.status.as_ref().unwrap();
        let targets = status
            .apps
            .iter()
            .filter(|item| item.state == ConnectionState::Checking)
            .map(|item| item.app)
            .collect::<Vec<_>>();
        (
            session,
            status.username.clone().unwrap_or_default(),
            inner.cancel.clone(),
            targets,
        )
    };
    for target in targets {
        let app = app.clone();
        let session = session.clone();
        let username = username.clone();
        let cancel = cancel.clone();
        tauri::async_runtime::spawn(async move {
            let result = Connection::connect(target, &session, &username, cancel).await;
            let state = app.state::<AuthState>();
            let mut inner = state.lock();
            if !inner.connecting(id) {
                return;
            }
            let (phase, message) = match result {
                Ok(connection) => {
                    inner.clients.insert(target, connection);
                    (ConnectionState::Connected, None)
                }
                Err(message) => (ConnectionState::Failed, Some(message)),
            };
            let mut status = inner.status.clone().unwrap();
            if let Some(item) = status.apps.iter_mut().find(|item| item.app == target) {
                item.state = phase;
                item.message = message;
            }
            let status = inner.update(status);
            drop(inner);
            publish(&app, status);
        });
    }
}
fn retry_failed(status: &mut Status) -> bool {
    if status
        .apps
        .iter()
        .any(|item| item.state == ConnectionState::Checking)
    {
        return false;
    }
    let mut changed = false;
    for item in &mut status.apps {
        if item.state == ConnectionState::Failed {
            item.state = ConnectionState::Checking;
            item.message = None;
            changed = true;
        }
    }
    changed
}
#[tauri::command]
pub async fn auth_connect_apps(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> AuthResult<Status> {
    authorize(&window)?;
    let state = app.state::<AuthState>();
    let mut inner = state.lock();
    let mut status = inner.status.clone().ok_or("Please sign in first.")?;
    if status.state != Phase::SignedIn {
        return Err("Please sign in first.".into());
    }
    if !retry_failed(&mut status) {
        return Ok(status);
    }
    let id = inner.generation;
    let status = inner.update(status);
    drop(inner);
    publish(&app, status.clone());
    launch_apps(app, id);
    Ok(status)
}
fn launch(app: tauri::AppHandle, id: u64, mode: Mode) {
    if mode == Mode::Interactive {
        tauri::async_runtime::spawn_blocking(move || {
            if let Err(error) = run(&app, id) {
                finish(&app, id, Err(error), true);
            }
        });
    } else {
        let cancel = app.state::<AuthState>().lock().cancel.clone();
        tauri::async_runtime::spawn(async move {
            let result = match tauri::async_runtime::spawn_blocking(storage::load).await {
                Ok(Ok(Some(session))) => {
                    // Keep the seed on network failure without treating it as authenticated.
                    if !app.state::<AuthState>().lock().remember(id, &session) {
                        return;
                    }
                    match apps::portal_identity(&session, cancel).await {
                        Ok(body) => identity(&body).map(|status| (status, Some(session))),
                        Err(error) if error == apps::LOGIN_REQUIRED => {
                            Ok((Status::new(Phase::SignedOut), None))
                        }
                        Err(error) => Err(error),
                    }
                }
                Ok(Ok(None)) => Ok((Status::new(Phase::SignedOut), None)),
                Ok(Err(error)) => Err(error),
                Err(_) => Err("Could not load the saved ITS session.".into()),
            };
            tauri::async_runtime::spawn_blocking(move || finish(&app, id, result, false));
        });
    }
}
#[tauri::command]
pub async fn auth_get_status(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> AuthResult<Status> {
    authorize(&window)?;
    let state = app.state::<AuthState>();
    let mut inner = state.lock();
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
    let mut inner = state.lock();
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
        let mut inner = state.lock();
        if inner
            .status
            .as_ref()
            .is_some_and(|s| s.state == Phase::SigningOut)
        {
            return Err("ITS sign-out is in progress. Please wait.".into());
        }
        if !forget && !inner.status.as_ref().is_some_and(|s| s.state.busy()) {
            return Ok(inner
                .status
                .clone()
                .unwrap_or_else(|| Status::new(Phase::SignedOut)));
        }
        let old_id = inner.generation;
        inner.cancel_work();
        inner.session = None;
        // Serialize deletion with login commits so reset cannot erase a newer session.
        let cleared = if forget { storage::clear() } else { Ok(()) };
        let next = match &cleared {
            Ok(()) => Status::new(Phase::SignedOut),
            Err(error) => Status::error(error.clone()),
        };
        let status = inner.update(next);
        drop(inner);
        publish(&app, status.clone());
        if let Some(w) = app.get_webview_window(&label(old_id)) {
            let _ = w.close();
        }
        cleared.map(|()| status)
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
fn logout_status(
    previous: Status,
    revoked: AuthResult<()>,
    clear: impl FnOnce() -> AuthResult<()>,
) -> Status {
    match revoked.and_then(|()| clear()) {
        Ok(()) => Status {
            message: Some("Signed out of ITS.".into()),
            ..Status::new(Phase::SignedOut)
        },
        Err(message) => Status {
            message: Some(message),
            ..previous
        },
    }
}
#[tauri::command]
pub async fn auth_forget_session(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> AuthResult<Status> {
    authorize(&window)?;
    reset(app, true).await
}
#[tauri::command]
pub async fn auth_logout(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> AuthResult<Status> {
    authorize(&window)?;
    let state = app.state::<AuthState>();
    let mut inner = state.lock();
    let mut previous = inner
        .status
        .clone()
        .unwrap_or_else(|| Status::new(Phase::SignedOut));
    for item in &mut previous.apps {
        item.state = ConnectionState::Failed;
        item.message = Some("Reconnect to verify app sign-in.".into());
    }
    let session = inner.session.clone();
    let Some((id, mut status)) = inner.begin(Phase::SigningOut) else {
        return Ok(previous);
    };
    status.name = previous.name.clone();
    status.username = previous.username.clone();
    status.apps = previous.apps.clone();
    inner.status = Some(status.clone());
    drop(inner);
    publish(&app, status.clone());
    let cancel = state.lock().cancel.clone();
    tauri::async_runtime::spawn(async move {
        let result=match session {
            Some(session)=>logout::invalidate(session,cancel).await,
            None=>Err("No saved session is available to invalidate. Reset saved sign-in to remove local data.".into()),
        };
        tauri::async_runtime::spawn_blocking(move || {
            let state = app.state::<AuthState>();
            let mut inner = state.lock();
            if !inner.current(id) {
                return;
            }
            let next = logout_status(previous, result, storage::clear);
            if next.state == Phase::SignedOut {
                inner.session = None;
            }
            let next = inner.finish(id, next).unwrap();
            drop(inner);
            publish(&app, next);
        });
    });
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_restore_keeps_the_seed_without_granting_authentication() {
        let session = Session {
            version: 1,
            cookies: vec![(
                "https://my.its.ac.id/".into(),
                "session=test; Secure; HttpOnly".into(),
            )],
        };
        let mut inner = Inner::default();
        let (id, _) = inner.begin(Phase::Checking).unwrap();
        assert!(inner.remember(id, &session));
        assert!(!inner.connecting(id));
        inner.finish(id, Status::error("Temporary network error".into()));
        assert!(inner.session.is_some());
        let (next, _) = inner.begin(Phase::SigningIn).unwrap();
        assert!(inner.session.is_some());
        assert!(!inner.remember(id, &session));
        assert!(inner.current(next));
    }
    #[test]
    fn retries_preserve_successful_connections() {
        let mut status = Status {
            apps: apps::checking(),
            ..Status::new(Phase::SignedIn)
        };
        status.apps[0].state = ConnectionState::Connected;
        status.apps[1].state = ConnectionState::Failed;
        assert!(!retry_failed(&mut status));
        status.apps[2].state = ConnectionState::Connected;
        assert!(retry_failed(&mut status));
        assert_eq!(status.apps[0].state, ConnectionState::Connected);
        assert_eq!(status.apps[1].state, ConnectionState::Checking);
        assert_eq!(status.apps[2].state, ConnectionState::Connected);
    }
    #[test]
    fn replacing_an_attempt_cancels_its_network_work() {
        let mut inner = Inner::default();
        let (id, _) = inner.begin(Phase::SigningIn).unwrap();
        let cancel = inner.cancel.clone();
        inner.finish(id, Status::new(Phase::SignedIn));
        inner.begin(Phase::SigningOut).unwrap();
        assert!(cancel.is_cancelled());
        assert!(!inner.cancel.is_cancelled());
    }
    #[test]
    fn snapshots_keep_only_the_exact_central_sso_cookie() {
        let session = Session {
            version: 1,
            cookies: vec![
                (
                    "https://portal.its.ac.id/".into(),
                    "myits_portal_api_session=portal; Path=/".into(),
                ),
                (
                    "https://its.ac.id/".into(),
                    "AMCV_AdobeOrg=analytics; Path=/".into(),
                ),
                (
                    "https://mia.its.ac.id/".into(),
                    "myits_academics_session=mia; Path=/".into(),
                ),
                (
                    "https://my.its.ac.id/".into(),
                    "session=central; Domain=my.its.ac.id; Path=/".into(),
                ),
            ],
        };
        let minimal = session.central_only().unwrap();
        assert_eq!(minimal.cookies.len(), 1);
        assert_eq!(minimal.cookies[0].0, "https://my.its.ac.id/");
        let cookie = tauri::webview::Cookie::parse(minimal.cookies[0].1.clone()).unwrap();
        assert_eq!(cookie.name(), "session");
        assert_eq!(cookie.value(), "central");
        assert!(cookie.domain().is_none());
        assert_eq!(cookie.secure(), Some(true));
        assert_eq!(cookie.http_only(), Some(true));
        for (origin, raw) in [
            ("http://my.its.ac.id/", "session=test"),
            ("https://portal.its.ac.id/", "session=test"),
            ("https://my.its.ac.id/", "session=test; Domain=.its.ac.id"),
            ("https://my.its.ac.id/", "session=test; Path=/authorize"),
            ("https://my.its.ac.id/", "session=test; Max-Age=0"),
            (
                "https://my.its.ac.id/",
                "session=test; Expires=Thu, 01 Jan 1970 00:00:00 GMT",
            ),
        ] {
            assert!(
                Session {
                    version: 1,
                    cookies: vec![(origin.into(), raw.into())]
                }
                .central_only()
                .is_err()
            );
        }
    }
    #[test]
    fn sign_out_and_reset_reject_late_app_connections() {
        let mut inner = Inner::default();
        let (id, _) = inner.begin(Phase::SigningIn).unwrap();
        inner.finish(id, Status::new(Phase::SignedIn));
        assert!(inner.connecting(id));
        let (logout, _) = inner.begin(Phase::SigningOut).unwrap();
        assert!(!inner.connecting(id));
        assert!(!inner.connecting(logout));
        inner.finish(logout, Status::new(Phase::SignedOut));
        assert!(!inner.connecting(id));
    }
    #[test]
    fn verification_retries_back_off_and_are_bounded() {
        assert_eq!(probe_delay(0), Duration::from_secs(1));
        assert_eq!(probe_delay(1), Duration::from_secs(2));
        assert_eq!(probe_delay(4), Duration::from_secs(16));
        assert_eq!(probe_delay(100), Duration::from_secs(30));
    }
    #[test]
    fn logout_only_clears_local_state_after_confirmed_remote_revocation() {
        let previous = Status {
            username: Some("test-student".into()),
            ..Status::new(Phase::SignedIn)
        };
        let failed = logout_status(previous.clone(), Err("Server unavailable".into()), || {
            panic!("Must retain the vault to retry")
        });
        assert_eq!(failed.state, Phase::SignedIn);
        assert_eq!(failed.username, previous.username);
        let failed = logout_status(previous.clone(), Ok(()), || {
            Err("Vault deletion failed".into())
        });
        assert_eq!(failed.state, Phase::SignedIn);
        let done = logout_status(previous, Ok(()), || Ok(()));
        assert_eq!(done.state, Phase::SignedOut);
        assert!(done.username.is_none());
    }
    #[test]
    fn sign_out_blocks_new_login_and_stale_success() {
        let mut inner = Inner::default();
        let (old, _) = inner.begin(Phase::SigningIn).unwrap();
        inner.finish(old, Status::new(Phase::SignedIn));
        let (id, _) = inner.begin(Phase::SigningOut).unwrap();
        assert!(inner.begin(Phase::SigningIn).is_none());
        assert!(inner.finish(old, Status::new(Phase::SignedIn)).is_none());
        assert!(inner.finish(id, Status::new(Phase::SignedOut)).is_some());
    }
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
