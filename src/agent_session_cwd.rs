//! Best-effort cross-project protection for native session reports and cold restore.
//! A matching directory is not proof of pane ownership. Missing metadata preserves
//! compatibility; only an observed conflict blocks automatic binding/restoration.
//! Keep filesystem access at those two boundaries, never in state/render loops.

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use crate::agent_resume::{AgentSessionRef, AgentSessionRefKind};

const MAX_METADATA_BYTES: u64 = 1024 * 1024;
const MAX_METADATA_LINES: usize = 256;
const MAX_STORE_ENTRIES: usize = 16_384;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CwdConflict {
    pub session_directories: Vec<PathBuf>,
}

pub(crate) fn conflict(
    source: &str,
    agent: &str,
    session: &AgentSessionRef,
    transcript: Option<&Path>,
    pane_cwd: &Path,
) -> Option<CwdConflict> {
    if !crate::agent_resume::is_official_agent_source(source, agent)
        || session.kind != AgentSessionRefKind::Id
        || !matches!(agent, "codex" | "claude" | "qodercli")
    {
        return None;
    }
    let directories = transcript
        .and_then(|path| read_directories(path, agent, &session.value))
        .or_else(|| {
            let root = match agent {
                "codex" => crate::integration::session_store_codex_dir().ok()?,
                "claude" => crate::integration::session_store_claude_dir().ok()?,
                "qodercli" => crate::integration::session_store_qoder_dir().ok()?,
                _ => return None,
            };
            find_directories(&root, agent, &session.value)
        })?;
    compare_directories(directories, pane_cwd)
}

fn compare_directories(directories: Vec<PathBuf>, pane_cwd: &Path) -> Option<CwdConflict> {
    if directories.is_empty() || !pane_cwd.is_absolute() {
        return None;
    }
    let pane_cwd = fs::canonicalize(pane_cwd).unwrap_or_else(|_| pane_cwd.to_path_buf());
    if directories
        .iter()
        .any(|cwd| fs::canonicalize(cwd).unwrap_or_else(|_| cwd.clone()) == pane_cwd)
    {
        None
    } else {
        Some(CwdConflict {
            session_directories: directories,
        })
    }
}

fn read_directories(path: &Path, agent: &str, session_id: &str) -> Option<Vec<PathBuf>> {
    if !path.is_absolute() {
        return None;
    }
    // Avoid reading devices/FIFOs or entire, potentially very large transcripts.
    if !fs::metadata(path).ok()?.is_file() {
        return None;
    }
    let file = File::open(path).ok()?;
    let reader = BufReader::new(file.take(MAX_METADATA_BYTES));
    for line in reader.lines().take(MAX_METADATA_LINES) {
        let line = line.ok()?;
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let metadata = if agent == "codex" {
            if value.get("type").and_then(|v| v.as_str()) != Some("session_meta") {
                continue;
            }
            let payload = value.get("payload")?;
            if payload.get("id").and_then(|id| id.as_str()) != Some(session_id) {
                return None;
            }
            payload
        } else {
            if value.get("sessionId").and_then(|id| id.as_str()) != Some(session_id) {
                continue;
            }
            &value
        };
        if agent == "qodercli"
            && metadata.get("type").and_then(|v| v.as_str()) == Some("workspace-directories")
        {
            let directories: Option<Vec<_>> = metadata
                .get("directories")?
                .as_array()?
                .iter()
                .map(|v| absolute_path(v.as_str()?))
                .collect();
            if let Some(directories) = directories.filter(|dirs| !dirs.is_empty()) {
                return Some(directories);
            }
        }
        if let Some(cwd) = metadata
            .get("cwd")
            .and_then(|v| v.as_str())
            .and_then(absolute_path)
        {
            return Some(vec![cwd]);
        }
    }
    None
}

fn absolute_path(value: &str) -> Option<PathBuf> {
    let path = PathBuf::from(value);
    path.is_absolute().then_some(path)
}

fn find_directories(root: &Path, agent: &str, session_id: &str) -> Option<Vec<PathBuf>> {
    // IDs are opaque, but must be safe filename components for store lookup.
    if session_id.is_empty()
        || !session_id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return None;
    }
    let mut pending = if agent == "codex" {
        vec![
            (root.join("archived_sessions"), 3),
            (root.join("sessions"), 3),
        ]
    } else {
        vec![(root.join("projects"), 1)]
    };
    let filename = format!("{session_id}.jsonl");
    let suffix = format!("-{session_id}.jsonl");
    let mut visited = 0;
    while let Some((directory, depth)) = pending.pop() {
        let Ok(entries) = fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            visited += 1;
            if visited > MAX_STORE_ENTRIES {
                return None;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() && depth > 0 {
                pending.push((entry.path(), depth - 1));
            } else if kind.is_file() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name == filename
                    || (agent == "codex" && name.starts_with("rollout-") && name.ends_with(&suffix))
                {
                    if let Some(directories) = read_directories(&entry.path(), agent, session_id) {
                        return Some(directories);
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
pub(crate) struct TestDir(PathBuf);

#[cfg(test)]
impl TestDir {
    pub(crate) fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "herdr-session-cwd-{}-{stamp}-{id}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

#[cfg(test)]
impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_cwd_guard_reads_native_stores_and_rejects_foreign_project() {
        let tmp = TestDir::new();
        let cwd = tmp.path().join("platform");
        let foreign = tmp.path().join("Dorm");
        for agent in ["codex", "claude", "qodercli"] {
            let root = tmp.path().join(agent);
            let path = if agent == "codex" {
                root.join("sessions/2026/11/11/rollout-date-session.jsonl")
            } else {
                root.join("projects/project/session.jsonl")
            };
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let record = if agent == "codex" {
                serde_json::json!({"type":"session_meta","payload":{"id":"session","cwd":cwd}})
            } else if agent == "claude" {
                serde_json::json!({"type":"user","sessionId":"session","cwd":cwd})
            } else {
                serde_json::json!({"type":"workspace-directories","sessionId":"session","directories":[cwd]})
            };
            fs::write(
                &path,
                format!("{{\"type\":\"queue-operation\"}}\n{record}\n"),
            )
            .unwrap();
            let dirs = find_directories(&root, agent, "session").unwrap();
            assert!(compare_directories(dirs.clone(), &cwd).is_none());
            assert!(compare_directories(dirs, &foreign).is_some());
            assert!(read_directories(&path, agent, "other-session").is_none());
        }
    }

    #[test]
    fn session_cwd_guard_bounds_reads_and_preserves_unknown_metadata() {
        let tmp = TestDir::new();
        let path = tmp.path().join("session.jsonl");
        fs::write(&path, "x".repeat(MAX_METADATA_BYTES as usize + 1)).unwrap();
        assert!(read_directories(&path, "codex", "session").is_none());
        fs::write(&path, "{}\ninvalid\n").unwrap();
        assert!(read_directories(&path, "claude", "session").is_none());
        assert!(find_directories(tmp.path(), "codex", "../session").is_none());
        let session = AgentSessionRef::id("session").unwrap();
        assert!(conflict("custom:codex", "codex", &session, Some(&path), tmp.path()).is_none());
        assert!(conflict(
            "herdr:opencode",
            "opencode",
            &session,
            Some(&path),
            tmp.path()
        )
        .is_none());
    }

    #[test]
    fn session_cwd_guard_qoder_accepts_only_explicit_workspaces() {
        let tmp = TestDir::new();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        let path = tmp.path().join("session.jsonl");
        fs::write(&path, serde_json::json!({"type":"workspace-directories", "sessionId":"session", "directories":[a,b]}).to_string()).unwrap();
        let dirs = read_directories(&path, "qodercli", "session").unwrap();
        assert!(compare_directories(dirs.clone(), &b).is_none());
        assert!(compare_directories(dirs, &b.join("child")).is_some());
    }

    #[cfg(unix)]
    #[test]
    fn session_cwd_guard_accepts_symlink_aliases() {
        let tmp = TestDir::new();
        let cwd = tmp.path().join("real");
        let alias = tmp.path().join("alias");
        fs::create_dir(&cwd).unwrap();
        std::os::unix::fs::symlink(&cwd, &alias).unwrap();
        assert!(compare_directories(vec![cwd], &alias).is_none());
    }
}
