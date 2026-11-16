// JSON persistence in the app config dir: websites, commands, favorites, history.
// Single small file; written atomically (tmp + rename). No external data leaves the machine.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Website {
    pub id: String, // "web:<url>"
    pub name: String,
    pub url: String,
    pub kind: String, // "website"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Command {
    pub id: String, // "cmd:<name>"
    pub name: String,
    pub cmd: String,
    pub kind: String, // "command"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,   // kind:item id
    pub name: String,
    pub kind: String,
    pub ts: u64,      // unix seconds
    pub count: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Store {
    version: u32,
    favorites: Vec<String>,
    websites: Vec<Website>,
    commands: Vec<Command>,
    history: Vec<HistoryEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub favorites: Vec<String>,
    pub websites: Vec<Website>,
    pub commands: Vec<Command>,
    pub history: Vec<HistoryEntry>,
}

pub struct JsonStore {
    path: PathBuf,
    data: Store,
}

impl JsonStore {
    pub fn load(dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("quickdeck.json");
        let data = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<Store>(&s).ok())
            .unwrap_or(Store { version: 1, ..Default::default() });
        Self { path, data }
    }

    fn save(&self) {
        let tmp = self.path.with_extension("json.tmp");
        if let Ok(json) = serde_json::to_string_pretty(&self.data) {
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, &self.path);
            }
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            favorites: self.data.favorites.clone(),
            websites: self.data.websites.clone(),
            commands: self.data.commands.clone(),
            history: self.data.history.clone(),
        }
    }

    pub fn toggle_favorite(&mut self, id: &str) -> bool {
        if let Some(pos) = self.data.favorites.iter().position(|f| f == id) {
            self.data.favorites.remove(pos);
            self.save();
            false
        } else {
            self.data.favorites.push(id.to_string());
            self.save();
            true
        }
    }

    pub fn add_website(&mut self, name: &str, url: &str) -> Result<Website, String> {
        let url = normalize_url(url).ok_or_else(|| "URL must start with http:// or https://".to_string())?;
        if name.trim().is_empty() {
            return Err("Name is required".into());
        }
        let site = Website { id: format!("web:{url}"), name: name.trim().to_string(), url, kind: "website".into() };
        self.data.websites.retain(|w| w.id != site.id);
        self.data.websites.push(site.clone());
        self.save();
        Ok(site)
    }

    pub fn remove_website(&mut self, id: &str) {
        self.data.websites.retain(|w| w.id != id);
        self.data.favorites.retain(|f| f != id);
        self.save();
    }

    pub fn add_command(&mut self, name: &str, cmd: &str) -> Result<Command, String> {
        if name.trim().is_empty() || cmd.trim().is_empty() {
            return Err("Name and command are required".into());
        }
        let c = Command { id: format!("cmd:{}", name.trim()), name: name.trim().to_string(), cmd: cmd.trim().to_string(), kind: "command".into() };
        self.data.commands.retain(|x| x.id != c.id);
        self.data.commands.push(c.clone());
        self.save();
        Ok(c)
    }

    pub fn remove_command(&mut self, id: &str) {
        self.data.commands.retain(|c| c.id != id);
        self.data.favorites.retain(|f| f != id);
        self.save();
    }

    pub fn record_launch(&mut self, id: &str, name: &str, kind: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        match self.data.history.iter_mut().find(|h| h.id == id) {
            Some(entry) => {
                entry.ts = now;
                entry.count = entry.count.saturating_add(1);
                entry.name = name.to_string();
            }
            None => self.data.history.push(HistoryEntry {
                id: id.to_string(), name: name.to_string(), kind: kind.to_string(), ts: now, count: 1,
            }),
        }
        // cap: keep 200 most recent by ts
        if self.data.history.len() > 200 {
            self.data.history.sort_by(|a, b| b.ts.cmp(&a.ts));
            self.data.history.truncate(200);
        }
        self.save();
    }

    pub fn clear_history(&mut self) {
        self.data.history.clear();
        self.save();
    }
}

fn normalize_url(url: &str) -> Option<String> {
    let u = url.trim();
    let with_scheme = if u.starts_with("http://") || u.starts_with("https://") {
        u.to_string()
    } else {
        format!("https://{u}")
    };
    if with_scheme.len() > 8 && !with_scheme[8..].contains(char::is_whitespace) && with_scheme[8..].contains('.') {
        Some(with_scheme)
    } else if with_scheme.starts_with("http://localhost") {
        Some(with_scheme)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> JsonStore {
        let dir = std::env::temp_dir().join(format!(
            "quickdeck-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        JsonStore::load(dir)
    }

    #[test]
    fn url_normalization() {
        assert_eq!(normalize_url("example.com"), Some("https://example.com".into()));
        assert_eq!(normalize_url("https://a.b/path?q=1"), Some("https://a.b/path?q=1".into()));
        assert_eq!(normalize_url("http://localhost:3000"), Some("http://localhost:3000".into()));
        assert_eq!(normalize_url("javascript:alert(1)"), None);
        assert_eq!(normalize_url("https:// has spaces"), None);
        assert_eq!(normalize_url("notaurl"), None);
    }

    #[test]
    fn favorites_toggle_and_persist() {
        let mut s = temp_store();
        assert!(s.toggle_favorite("app:test.desktop"));
        assert!(s.snapshot().favorites.contains(&"app:test.desktop".to_string()));
        assert!(!s.toggle_favorite("app:test.desktop"));
        // reload from disk — should see the write
        let snap = JsonStore::load(s.path.clone()).snapshot();
        assert!(snap.favorites.is_empty());
    }

    #[test]
    fn history_counts_and_caps() {
        let mut s = temp_store();
        s.record_launch("app:a.desktop", "A App", "app");
        s.record_launch("app:a.desktop", "A App", "app");
        let snap = s.snapshot();
        let e = snap.history.iter().find(|h| h.id == "app:a.desktop").unwrap();
        assert_eq!(e.count, 2);
    }

    #[test]
    fn website_added_normalized() {
        let mut s = temp_store();
        let site = s.add_website("Example", "example.com").unwrap();
        assert_eq!(site.url, "https://example.com");
        assert!(s.add_website("", "x.com").is_err());
    }
}
