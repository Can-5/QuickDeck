// App discovery per platform.
// Linux: parse XDG .desktop files. Windows: Start Menu / Desktop .lnk names.
// macOS: /Applications *.app bundles (+ Info.plist display name when readable).
// Icons are intentionally not decoded — the frontend uses a glyph per kind.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppEntry {
    /// stable id, e.g. "app:firefox.desktop" or absolute app/lnk path on win/mac
    pub id: String,
    pub name: String,
    pub comment: String,
    pub keywords: Vec<String>,
    pub icon: String,
    pub kind: String, // "app"
    pub source: String,
}

#[cfg(target_os = "linux")]
pub fn discover_apps() -> Vec<AppEntry> {
    let mut dirs: Vec<PathBuf> = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(&home).join(".local/share/applications"));
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_DIRS") {
        for part in xdg.split(':') {
            if !part.is_empty() {
                dirs.push(PathBuf::from(part).join("applications"));
            }
        }
    }
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for dir in dirs {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            if let Some(app) = parse_desktop_file(&path) {
                if seen.insert(app.id.clone()) {
                    out.push(app);
                }
            }
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

#[cfg(target_os = "linux")]
fn parse_desktop_file(path: &Path) -> Option<AppEntry> {
    let raw = std::fs::read_to_string(path).ok()?;
    let mut in_entry = false;
    let mut name = String::new();
    let mut comment = String::new();
    let mut exec = String::new();
    let mut icon = String::new();
    let mut keywords: Vec<String> = Vec::new();
    let mut no_display = false;
    let mut hidden = false;
    let mut type_ok = true; // only Type=Application
    for line in raw.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry || line.starts_with('#') {
            continue;
        }
        let (key, value) = line.split_once('=')?;
        let key = key.trim();
        let value = value.trim();
        match key {
            "Name" if name.is_empty() => name = value.to_string(),
            "Comment" if comment.is_empty() => comment = value.to_string(),
            "Exec" if exec.is_empty() => exec = value.to_string(),
            "Icon" if icon.is_empty() => icon = value.to_string(),
            "Keywords" => keywords = value.split(';').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect(),
            "NoDisplay" => no_display = value.eq_ignore_ascii_case("true"),
            "Hidden" => hidden = value.eq_ignore_ascii_case("true"),
            "Type" => type_ok = value == "Application",
            _ => {}
        }
    }
    if name.is_empty() || exec.is_empty() || no_display || hidden || !type_ok {
        return None;
    }
    let file_name = path.file_name()?.to_str()?.to_string();
    Some(AppEntry {
        id: format!("app:{file_name}"),
        name,
        comment,
        keywords,
        icon,
        kind: "app".into(),
        source: exec,
    })
}

/// Extract the .desktop id for gtk-launch, and strip field codes from Exec for fallback.
#[cfg(target_os = "linux")]
pub fn desktop_id(id: &str) -> Option<&str> {
    id.strip_prefix("app:")
}
#[cfg(target_os = "linux")]
pub fn strip_field_codes(exec: &str) -> Vec<String> {
    tokenize_exec(exec)
        .into_iter()
        .filter(|tok| !matches!(tok.as_str(),
            "%f" | "%F" | "%u" | "%U" | "%d" | "%D" | "%i" | "%c" | "%k" | "%v" | "%m" | "%n"))
        .collect()
}

/// Small shell-ish tokenizer: splits on whitespace, respects single/double quotes.
#[cfg(target_os = "linux")]
fn tokenize_exec(exec: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = exec.chars().peekable();
    let mut quote: Option<char> = None;
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                if c == q { quote = None } else { cur.push(c) }
            }
            None => match c {
                '\'' | '"' => quote = Some(c),
                c if c.is_whitespace() => {
                    if !cur.is_empty() { out.push(std::mem::take(&mut cur)); }
                }
                '\\' => { if let Some(n) = chars.next() { cur.push(n) } }
                c => cur.push(c),
            },
        }
    }
    if !cur.is_empty() { out.push(cur) }
    out
}

#[cfg(target_os = "windows")]
pub fn discover_apps() -> Vec<AppEntry> {
    let mut roots: Vec<PathBuf> = Vec::new();
    for var in ["PROGRAMDATA", "APPDATA"] {
        if let Ok(v) = std::env::var(var) {
            roots.push(PathBuf::from(v).join("Microsoft\\Windows\\Start Menu\\Programs"));
        }
    }
    if let Ok(v) = std::env::var("PUBLIC") {
        roots.push(PathBuf::from(v).join("Desktop"));
    }
    if let Ok(v) = std::env::var("USERPROFILE") {
        roots.push(PathBuf::from(&v).join("Desktop"));
        roots.push(PathBuf::from(&v).join("AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs"));
    }
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for root in roots {
        collect_lnk(&root, 0, &mut out, &mut seen);
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}
#[cfg(target_os = "windows")]
fn collect_lnk(dir: &Path, depth: u32, out: &mut Vec<AppEntry>, seen: &mut std::collections::HashSet<String>) {
    if depth > 4 { return; }
    let entries = match std::fs::read_dir(dir) { Ok(e) => e, Err(_) => return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_lnk(&path, depth + 1, out, seen);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("lnk")).unwrap_or(false) {
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown").to_string();
            let id = format!("win:{}", path.to_string_lossy());
            if seen.insert(name.to_lowercase()) {
                out.push(AppEntry {
                    id, name, comment: String::new(),
                    keywords: vec![], icon: String::new(),
                    kind: "app".into(), source: path.to_string_lossy().into_owned(),
                });
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub fn discover_apps() -> Vec<AppEntry> {
    let mut roots: Vec<PathBuf> = vec![PathBuf::from("/Applications")];
    if let Ok(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(&home).join("Applications"));
    }
    let mut out = Vec::new();
    for root in roots {
        let entries = match std::fs::read_dir(&root) { Ok(e) => e, Err(_) => continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("app") { continue; }
            let display = plist_display_name(&path).unwrap_or_else(|| {
                path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown").to_string()
            });
            out.push(AppEntry {
                id: format!("mac:{}", path.to_string_lossy()),
                name: display,
                comment: String::new(),
                keywords: vec![],
                icon: String::new(),
                kind: "app".into(),
                source: path.to_string_lossy().into_owned(),
            });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}
#[cfg(target_os = "macos")]
fn plist_display_name(app: &Path) -> Option<String> {
    let plist_path = app.join("Contents/Info.plist");
    let raw = std::fs::read_to_string(plist_path).ok()?;
    for key in ["CFBundleDisplayName", "CFBundleName"] {
        if let Some(pos) = raw.find(&format!("<key>{key}</key>")) {
            if let Some(start) = raw[pos..].find("<string>") {
                let s = pos + start + "<string>".len();
                if let Some(end) = raw[s..].find("</string>") {
                    let val = raw[s..s + end].trim();
                    if !val.is_empty() { return Some(val.to_string()); }
                }
            }
        }
    }
    None
}

// Tests live with each platform impl; these cover the parser (Linux path is what CI runs).
#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn strips_field_codes_and_quotes() {
        assert_eq!(strip_field_codes("firefox %u"), vec!["firefox"]);
        assert_eq!(strip_field_codes("code --new-window %F"), vec!["code", "--new-window"]);
        assert_eq!(strip_field_codes("env \"MY VAR\"=1 app %U"), vec!["env", "MY VAR=1", "app"]);
        assert_eq!(strip_field_codes("foo bar%%"), vec!["foo", "bar%%"]);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn tokenizes_quotes() {
        assert_eq!(tokenize_exec("a 'b c' \"d e\" f"), vec!["a", "b c", "d e", "f"]);
    }
}
