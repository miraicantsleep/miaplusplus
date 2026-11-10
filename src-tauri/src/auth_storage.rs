//! Private, encrypted ITS session storage. Never exposed through frontend IPC.
use super::auth::{Session, its_host};
use tauri::Manager;
use tauri_plugin_stronghold::stronghold::Stronghold;
use zeroize::Zeroizing;
const STORE_ERROR: &str = "Could not access secure session storage. Please try again.";
fn path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|_| STORE_ERROR)?
        .join("auth");
    std::fs::create_dir_all(&dir).map_err(|_| STORE_ERROR)?;
    Ok(dir.join("session.hold"))
}

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new("com.mirai.miaplusplus", "its-session-key-v1")
        .map_err(|_| STORE_ERROR.into())
}

fn key(create: bool) -> Result<Zeroizing<Vec<u8>>, String> {
    let entry = entry()?;
    match entry.get_secret() {
        Ok(bytes) => Ok(Zeroizing::new(bytes)),
        Err(keyring::Error::NoEntry) if create => {
            let mut bytes = Zeroizing::new(vec![0u8; 32]);
            getrandom::fill(&mut bytes).map_err(|_| STORE_ERROR)?;
            #[cfg(target_os = "windows")]
            write_windows_key(&bytes)?;
            #[cfg(not(target_os = "windows"))]
            entry.set_secret(&bytes).map_err(|_| STORE_ERROR)?;
            Ok(bytes)
        }
        Err(_) => Err(STORE_ERROR.into()),
    }
}

// Create with local persistence directly; never briefly write a roaming key.
#[cfg(target_os = "windows")]
fn write_windows_key(bytes: &[u8]) -> Result<(), String> {
    use windows_sys::Win32::Security::Credentials::{
        CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC, CREDENTIALW, CredWriteW,
    };
    let target: Vec<u16> = "its-session-key-v1.com.mirai.miaplusplus"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut credential: CREDENTIALW = unsafe { std::mem::zeroed() };
    credential.Type = CRED_TYPE_GENERIC;
    credential.TargetName = target.as_ptr().cast_mut();
    credential.Persist = CRED_PERSIST_LOCAL_MACHINE;
    credential.CredentialBlobSize = bytes.len() as u32;
    credential.CredentialBlob = bytes.as_ptr().cast_mut();
    // CredWriteW copies the pointed-to buffers; they remain alive for this call.
    if unsafe { CredWriteW(&credential, 0) } == 0 {
        Err(STORE_ERROR.into())
    } else {
        Ok(())
    }
}

pub(super) fn save(app: &tauri::AppHandle, session: &Session) -> Result<(), String> {
    let vault = Stronghold::new(path(app)?, key(true)?.to_vec()).map_err(|_| STORE_ERROR)?;
    let client = vault
        .load_client(b"its-session")
        .or_else(|_| vault.create_client(b"its-session"))
        .map_err(|_| STORE_ERROR)?;
    let data = Zeroizing::new(serde_json::to_vec(session).map_err(|_| STORE_ERROR)?);
    client
        .store()
        .insert(b"session-v1".to_vec(), data.to_vec(), None)
        .map_err(|_| STORE_ERROR)?;
    vault.save().map_err(|_| STORE_ERROR.into())
}

pub(super) fn load(app: &tauri::AppHandle) -> Result<Option<Session>, String> {
    let file = path(app)?;
    if !file.exists() {
        return Ok(None);
    }
    let vault = Stronghold::new(file, key(false)?.to_vec()).map_err(|_| STORE_ERROR)?;
    let client = vault.load_client(b"its-session").map_err(|_| STORE_ERROR)?;
    let data = Zeroizing::new(
        client
            .store()
            .get(b"session-v1")
            .map_err(|_| STORE_ERROR)?
            .ok_or(STORE_ERROR)?,
    );
    let session: Session = serde_json::from_slice(&data).map_err(|_| STORE_ERROR)?;
    if session.version != 1 {
        return Err(STORE_ERROR.into());
    }
    Ok(Some(session))
}

pub(super) fn clear(app: &tauri::AppHandle) -> Result<(), String> {
    let file = path(app)?;
    if file.exists() {
        std::fs::remove_file(file).map_err(|_| STORE_ERROR)?;
    }
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(STORE_ERROR.into()),
    }
}

fn restored_cookie(origin: &str, raw: &str) -> Result<tauri::webview::Cookie<'static>, String> {
    let url = tauri::Url::parse(origin).map_err(|_| "Invalid stored session")?;
    let host = url
        .host_str()
        .filter(|h| its_host(h))
        .ok_or("Invalid stored session")?;
    if url.scheme() != "https" {
        return Err("Invalid stored session".into());
    }
    let mut cookie =
        tauri::webview::Cookie::parse(raw.to_owned()).map_err(|_| "Invalid stored cookie")?;
    if let Some(domain) = cookie.domain() {
        let domain = domain.trim_start_matches('.');
        if !its_host(domain) || !(host == domain || host.ends_with(&format!(".{domain}"))) {
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
    for (origin, raw) in &session.cookies {
        window
            .set_cookie(restored_cookie(origin, raw)?)
            .map_err(|_| "Could not restore the ITS session")?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restore_rejects_out_of_scope_cookies() {
        assert!(restored_cookie("http://my.its.ac.id/", "session=test").is_err());
        assert!(restored_cookie("https://evil.test/", "session=test").is_err());
        assert!(
            restored_cookie("https://my.its.ac.id/", "session=test; Domain=evil.test").is_err()
        );
        assert!(
            restored_cookie(
                "https://my.its.ac.id/",
                "session=test; Domain=portal.its.ac.id"
            )
            .is_err()
        );
        assert!(restored_cookie("https://my.its.ac.id/", "session=test; Secure; HttpOnly").is_ok());
        assert!(
            restored_cookie("https://my.its.ac.id/", "session=test; Domain=.its.ac.id").is_ok()
        );
    }
    #[test]
    fn encrypted_snapshot_roundtrip_and_wrong_key_rejection() {
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).unwrap();
        let path = std::env::temp_dir().join(format!("mia-auth-test-{nonce:x?}.hold"));
        {
            let vault = Stronghold::new(&path, vec![17; 32]).unwrap();
            let client = vault.create_client(b"its-session").unwrap();
            client
                .store()
                .insert(b"session-v1".to_vec(), b"test-cookie-secret".to_vec(), None)
                .unwrap();
            vault.save().unwrap();
        }
        assert!(
            !std::fs::read(&path)
                .unwrap()
                .windows(18)
                .any(|w| w == b"test-cookie-secret")
        );
        assert!(Stronghold::new(&path, vec![18; 32]).is_err());
        let vault = Stronghold::new(&path, vec![17; 32]).unwrap();
        assert!(
            vault
                .load_client(b"its-session")
                .unwrap()
                .store()
                .get(b"session-v1")
                .unwrap()
                .is_some()
        );
        std::fs::remove_file(path).unwrap();
    }
}
