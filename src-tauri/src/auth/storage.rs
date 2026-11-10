//! Native credential storage for the central ITS session.
use super::Session;
use std::sync::Mutex;
use zeroize::Zeroizing;

static STORAGE: Mutex<()> = Mutex::new(());
const SERVICE: &str = "com.mirai.miaplusplus";
const ACCOUNT: &str = "its-session-v2";
const MAX_SECRET: usize = 2560;
const STORE_ERROR: &str = "Could not access secure session storage. Please try again.";

fn entry(account: &str) -> Result<keyring::Entry, String> {
    if !cfg!(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios"
    )) {
        return Err("Secure session storage is unavailable on this platform.".into());
    }
    keyring::Entry::new(SERVICE, account).map_err(|_| STORE_ERROR.into())
}
fn write(account: &str, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_SECRET {
        return Err("The ITS session exceeds the secure storage limit.".into());
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Security::Credentials::{
            CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredWriteW,
        };
        let target: Vec<u16> = format!("{account}.{SERVICE}")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut credential: CREDENTIALW = unsafe { std::mem::zeroed() };
        credential.Type = CRED_TYPE_GENERIC;
        credential.TargetName = target.as_ptr().cast_mut();
        credential.Persist = CRED_PERSIST_LOCAL_MACHINE;
        credential.CredentialBlobSize = bytes.len() as u32;
        credential.CredentialBlob = bytes.as_ptr().cast_mut();
        // Copy synchronously; keep the session on this device.
        if unsafe { CredWriteW(&credential, 0) } == 0 {
            return Err(STORE_ERROR.into());
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    entry(account)?
        .set_secret(bytes)
        .map_err(|_| STORE_ERROR.into())
}

pub(super) struct PreparedSession(Zeroizing<Vec<u8>>);
impl PreparedSession {
    pub(super) fn commit(self) -> Result<(), String> {
        let _guard = STORAGE.lock().map_err(|_| STORE_ERROR)?;
        write(ACCOUNT, &self.0)
    }
}
pub(super) fn prepare(session: &Session) -> Result<PreparedSession, String> {
    let session = session.central_only()?;
    let bytes = Zeroizing::new(serde_json::to_vec(&session).map_err(|_| STORE_ERROR)?);
    if bytes.len() > MAX_SECRET {
        return Err("The ITS session exceeds the secure storage limit.".into());
    }
    Ok(PreparedSession(bytes))
}
pub(super) fn load() -> Result<Option<Session>, String> {
    let _guard = STORAGE.lock().map_err(|_| STORE_ERROR)?;
    match entry(ACCOUNT)?.get_secret() {
        Ok(bytes) => {
            let bytes = Zeroizing::new(bytes);
            if bytes.len() > MAX_SECRET {
                return Err(STORE_ERROR.into());
            }
            let session: Session = serde_json::from_slice(&bytes).map_err(|_| STORE_ERROR)?;
            session.central_only().map(Some)
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err(STORE_ERROR.into()),
    }
}
pub(super) fn clear() -> Result<(), String> {
    let _guard = STORAGE.lock().map_err(|_| STORE_ERROR)?;
    match entry(ACCOUNT)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(STORE_ERROR.into()),
    }
}

fn restored_cookie(origin: &str, raw: &str) -> Result<tauri::webview::Cookie<'static>, String> {
    let url = tauri::Url::parse(origin).map_err(|_| "Invalid stored session")?;
    let host = url
        .host_str()
        .filter(|h| *h == "my.its.ac.id")
        .ok_or("Invalid stored session")?;
    if url.scheme() != "https" {
        return Err("Invalid stored session".into());
    }
    let mut cookie =
        tauri::webview::Cookie::parse(raw.to_owned()).map_err(|_| "Invalid stored cookie")?;
    if let Some(domain) = cookie.domain() {
        if domain.trim_start_matches('.') != host {
            return Err("Invalid stored cookie domain".into());
        }
    } else {
        cookie.set_domain(host.to_owned());
    }
    Ok(cookie)
}
pub(super) fn restore_cookies(
    window: &tauri::WebviewWindow,
    session: &Session,
) -> Result<(), String> {
    let session = session.central_only()?;
    for (origin, raw) in &session.cookies {
        window
            .set_cookie(restored_cookie(origin, raw)?)
            .map_err(|_| "Could not restore the user session")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn session(value: &str) -> Session {
        Session {
            version: 1,
            cookies: vec![(
                "https://my.its.ac.id/".into(),
                format!("session={value}; Path=/; Secure; HttpOnly"),
            )],
        }
    }
    #[test]
    fn preparation_rejects_oversized_or_invalid_sessions() {
        assert!(prepare(&session(&"x".repeat(MAX_SECRET))).is_err());
        assert!(prepare(&session("")).is_err());
        assert!(prepare(&session("test")).is_ok());
    }
    #[test]
    fn restore_rejects_out_of_scope_cookies() {
        for (origin, raw) in [
            ("http://my.its.ac.id/", "session=test"),
            ("https://evil.test/", "session=test"),
            ("https://my.its.ac.id/", "session=test; Domain=evil.test"),
            (
                "https://my.its.ac.id/",
                "session=test; Domain=portal.its.ac.id",
            ),
            ("https://my.its.ac.id/", "session=test; Domain=.its.ac.id"),
        ] {
            assert!(restored_cookie(origin, raw).is_err());
        }
        assert!(restored_cookie("https://my.its.ac.id/", "session=test; Secure; HttpOnly").is_ok());
    }
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    #[test]
    fn native_store_roundtrip_and_replacement() {
        let _guard = STORAGE.lock().unwrap();
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).unwrap();
        let account = format!("test-{nonce:x?}");
        let entry = entry(&account).unwrap();
        struct Cleanup(keyring::Entry);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = self.0.delete_credential();
            }
        }
        let cleanup = Cleanup(entry);
        let before = prepare(&session("before")).unwrap();
        write(&account, &before.0).unwrap();
        drop(prepare(&session("cancelled")).unwrap());
        assert_eq!(cleanup.0.get_secret().unwrap(), *before.0);
        let after = prepare(&session("after")).unwrap();
        write(&account, &after.0).unwrap();
        assert_eq!(cleanup.0.get_secret().unwrap(), *after.0);
        cleanup.0.delete_credential().unwrap();
        assert!(matches!(
            cleanup.0.get_secret(),
            Err(keyring::Error::NoEntry)
        ));
    }
}
