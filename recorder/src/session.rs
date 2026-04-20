use anyhow::{bail, Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionInfo {
    pub pid: u32,
    pub session_dir: PathBuf,
    pub started_at: String,
}

fn domino_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    Ok(home.join(".domino"))
}

fn pid_file_path() -> Result<PathBuf> {
    Ok(domino_dir()?.join("current.pid"))
}

fn recordings_dir(out_dir: Option<&Path>) -> Result<PathBuf> {
    match out_dir {
        Some(p) => Ok(p.to_path_buf()),
        None => Ok(domino_dir()?.join("recordings")),
    }
}

pub fn ensure_domino_dir() -> Result<PathBuf> {
    let dir = domino_dir()?;
    if !dir.exists() {
        fs::create_dir_all(&dir).context("failed to create ~/.domino")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        }
    }
    Ok(dir)
}

fn is_process_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

pub fn read_active_session() -> Result<Option<SessionInfo>> {
    let pid_path = pid_file_path()?;
    if !pid_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&pid_path).context("failed to read PID file")?;
    let info: SessionInfo =
        serde_json::from_str(&content).context("failed to parse PID file JSON")?;

    if is_process_alive(info.pid) {
        Ok(Some(info))
    } else {
        tracing::warn!(pid = info.pid, "stale PID file detected, removing");
        fs::remove_file(&pid_path).ok();
        Ok(None)
    }
}

/// Acquires the session lock, checks no session is active, and creates the session directory.
/// Returns `(session_dir, started_at)`. Does NOT write the PID file — the caller
/// must call `write_pid_file` after forking so the daemon PID is recorded.
pub fn prepare_session(out_dir: Option<&Path>) -> Result<(PathBuf, String)> {
    ensure_domino_dir()?;

    let lock_path = domino_dir()?.join("session.lock");
    let lock_file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .context("failed to open session lock file")?;

    lock_file
        .try_lock_exclusive()
        .context("another Domino recorder start is already in progress")?;

    let result = prepare_session_inner(out_dir);

    drop(lock_file);
    result
}

fn prepare_session_inner(out_dir: Option<&Path>) -> Result<(PathBuf, String)> {
    if let Some(existing) = read_active_session()? {
        bail!(
            "A recording is already in progress (session: {}, PID: {}). Run /record stop first.",
            existing.session_dir.display(),
            existing.pid
        );
    }

    let now = chrono::Local::now();
    let session_name = now.format("%Y-%m-%d-%H%M").to_string();
    let session_dir = recordings_dir(out_dir)?.join(&session_name);
    fs::create_dir_all(&session_dir)
        .with_context(|| format!("failed to create session dir: {}", session_dir.display()))?;

    Ok((session_dir, now.to_rfc3339()))
}

pub fn write_pid_file(pid: u32, session_dir: &Path, started_at: &str) -> Result<SessionInfo> {
    let info = SessionInfo {
        pid,
        session_dir: session_dir.to_path_buf(),
        started_at: started_at.to_string(),
    };

    let pid_path = pid_file_path()?;
    let json = serde_json::to_string_pretty(&info)?;
    fs::write(&pid_path, &json).context("failed to write PID file")?;

    Ok(info)
}

#[allow(dead_code)]
pub fn create_session(out_dir: Option<&Path>) -> Result<SessionInfo> {
    let (session_dir, started_at) = prepare_session(out_dir)?;
    write_pid_file(std::process::id(), &session_dir, &started_at)
}

pub fn remove_pid_file() -> Result<()> {
    let pid_path = pid_file_path()?;
    if pid_path.exists() {
        fs::remove_file(&pid_path).context("failed to remove PID file")?;
    }
    Ok(())
}

pub fn stop_session() -> Result<SessionInfo> {
    let session = read_active_session()?.context("no active recording session")?;

    unsafe {
        libc::kill(session.pid as i32, libc::SIGTERM);
    }

    let pid_path = pid_file_path()?;
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(5) {
        if !is_process_alive(session.pid) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    if is_process_alive(session.pid) {
        tracing::warn!(
            pid = session.pid,
            "process did not exit after SIGTERM, sending SIGKILL"
        );
        unsafe {
            libc::kill(session.pid as i32, libc::SIGKILL);
        }
    }

    if pid_path.exists() {
        fs::remove_file(&pid_path).ok();
    }

    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_session_info_serialization() {
        let info = SessionInfo {
            pid: 12345,
            session_dir: PathBuf::from("/tmp/test"),
            started_at: "2026-04-15T14:23:00-07:00".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: SessionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pid, 12345);
    }

    #[test]
    fn test_is_process_alive_self() {
        assert!(is_process_alive(std::process::id()));
    }

    #[test]
    fn test_is_process_alive_nonexistent() {
        assert!(!is_process_alive(999_999_999));
    }

    #[test]
    fn test_stale_pid_detection() {
        let tmp = env::temp_dir().join("domino-test-stale");
        fs::create_dir_all(&tmp).unwrap();
        let pid_path = tmp.join("current.pid");

        let info = SessionInfo {
            pid: 999_999_999,
            session_dir: tmp.join("fake-session"),
            started_at: "2026-04-15T00:00:00Z".to_string(),
        };
        fs::write(&pid_path, serde_json::to_string(&info).unwrap()).unwrap();

        let content = fs::read_to_string(&pid_path).unwrap();
        let parsed: SessionInfo = serde_json::from_str(&content).unwrap();
        assert!(!is_process_alive(parsed.pid));

        fs::remove_dir_all(&tmp).ok();
    }
}
