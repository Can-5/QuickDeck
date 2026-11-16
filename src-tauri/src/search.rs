// Filesystem search for the launcher: home dir walk, hidden entries skipped,
// time- and node-capped so it never hangs the UI.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub id: String, // "file:/abs/path"
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub kind: String, // "file"
}

const SKIP_DIRS: &[&str] = &[
    "node_modules", ".git", "target", "dist", "build", ".cache", ".local/share/Trash",
    "snap", ".cargo", ".rustup", ".npm", ".pnpm-store", "__pycache__", ".venv", "venv",
];

pub fn search_files(query: &str, limit: usize) -> Vec<FileEntry> {
    let q = query.to_lowercase();
    if q.len() < 2 {
        return Vec::new();
    }
    let home = match std::env::var_os("HOME") {
        Some(h) => PathBuf::from(h),
        None => return Vec::new(),
    };
    let limit = limit.clamp(1, 50);
    let deadline = Instant::now() + Duration::from_millis(900);
    let mut visited = 0usize;
    let mut results: Vec<(u8, FileEntry)> = Vec::new();
    let mut stack: Vec<(PathBuf, u32)> = vec![(home.clone(), 0)];

    while let Some((dir, depth)) = stack.pop() {
        if Instant::now() > deadline || visited > 250_000 {
            break;
        }
        if depth > 6 {
            continue;
        }
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            visited += 1;
            if Instant::now() > deadline {
                break;
            }
            let path = entry.path();
            let name = match entry.file_name().into_string() {
                Ok(n) => n,
                Err(_) => continue,
            };
            let is_hidden = name.starts_with('.');
            let is_dir = path.is_dir();
            if is_dir {
                let rel = path.strip_prefix(&home).unwrap_or(&path).to_string_lossy().to_string();
                if !is_hidden && !SKIP_DIRS.iter().any(|s| rel.starts_with(s) || name == *s) {
                    stack.push((path.clone(), depth + 1));
                }
            }
            if is_hidden {
                continue;
            }
            let lower = name.to_lowercase();
            let rank: u8 = if lower == q {
                0
            } else if lower.starts_with(&q) {
                1
            } else if lower.contains(&q) {
                2
            } else {
                continue;
            };
            results.push((rank, FileEntry {
                id: format!("file:{}", path.to_string_lossy()),
                name,
                path: path.to_string_lossy().into_owned(),
                is_dir,
                kind: "file".into(),
            }));
            if results.len() >= limit * 4 {
                // keep scanning a bit to find better-ranked matches, then stop
                if results.iter().filter(|(r, _)| *r == 0).count() >= limit {
                    break;
                }
            }
        }
    }
    results.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase())));
    results.into_iter().take(limit).map(|(_, e)| e).collect()
}

pub fn is_probably_safe_path(path: &str) -> bool {
    // Never allow launching arbitrary relative/shell-looking paths from IPC.
    let p = Path::new(path);
    p.is_absolute() && !path.contains('\0')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_relative_and_nul() {
        assert!(!is_probably_safe_path("../etc/passwd"));
        assert!(!is_probably_safe_path("foo\0bar"));
        assert!(is_probably_safe_path("/tmp/ok.txt"));
    }

    #[test]
    fn short_queries_return_nothing() {
        assert!(search_files("a", 5).is_empty());
    }
}
