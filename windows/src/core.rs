// Profile model and storage. A Windows profile is not a copy of the app: it's a record here, a Start Menu
// shortcut with its own AppUserModelID and icon, and a data folder passed to the real app as --user-data-dir.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use unicode_segmentation::UnicodeSegmentation;

pub const PALETTE: [&str; 8] = ["#0A84FF", "#30D158", "#BF5AF2", "#FF375F", "#40C8E0", "#5E5CE6", "#FF9F0A", "#FF453A"];

/// How a profile's icon looks (same options as the macOS app).
#[derive(Clone, Debug, PartialEq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct Look {
    pub badge: String,             // 0–2 characters; empty = no badge
    pub color: String,             // "#RRGGBB"
    pub image_path: Option<String>, // custom base image instead of the app's icon
    pub glyph: bool,               // the whole icon is `badge` on a `color` tile
    pub framed: bool,              // shrink the icon into a grey frame
}

impl Default for Look {
    fn default() -> Self { Look::defaults_for("") }
}

impl Look {
    /// Same hash as the macOS app, so a profile gets the same color on both.
    pub fn defaults_for(name: &str) -> Look {
        let idx = name.chars().fold(0u64, |h, c| (h.wrapping_mul(31).wrapping_add(c as u64)) & 0xffff) as usize % PALETTE.len();
        let badge = name.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "?".into());
        Look { badge, color: PALETTE[idx].into(), image_path: None, glyph: false, framed: false }
    }
    pub fn max_badge(&self) -> usize { if self.glyph { 1 } else { 2 } }
}

pub fn graphemes(s: &str) -> usize { s.graphemes(true).count() }
/// Keep the last `n` characters (typing past the limit replaces, like the macOS editor).
pub fn last_graphemes(s: &str, n: usize) -> String {
    let g: Vec<&str> = s.graphemes(true).collect();
    g[g.len().saturating_sub(n)..].concat()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String, // stable across renames, so taskbar pins and the AppUserModelID keep working
    pub name: String,
    pub source: String, // "Claude", "ChatGPT"
    pub data_dir: PathBuf,
    #[serde(default)]
    pub look: Look,
}

impl Profile {
    pub fn display(&self) -> String { format!("{} – {}", self.source, self.name) }
    pub fn key(&self) -> String { format!("profile:{}", self.id) }
    pub fn aumid(&self) -> String { format!("AIGENT.Profile.{}", self.id) }
    /// File name changes with the look: Windows caches shortcut icons by path.
    pub fn icon_path(&self) -> PathBuf {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.look.hash(&mut h);
        icons_dir().join(format!("{}-{:08x}.ico", self.id, h.finish() as u32))
    }
    pub fn shortcut_path(&self) -> PathBuf { start_menu_dir().join(format!("{}.lnk", self.display())) }
}

pub fn app_key(name: &str) -> String { format!("app:{name}") }

// MARK: Apps and processes

/// An app AIGENT can make profiles of. Installs are found by exe path (classic installers) or MSIX package.
pub struct Known { pub name: &'static str, pub exes: &'static [&'static str], pub package: &'static str, pub publisher: &'static str }
// ponytail: hardcoded list, like the macOS app; add any other Electron app that takes --user-data-dir.
pub const KNOWN: &[Known] = &[
    Known { name: "Claude", exes: &[r"AnthropicClaude\claude.exe", r"Programs\Claude\Claude.exe"], package: "claude", publisher: "Anthropic" },
    Known { name: "ChatGPT", exes: &[r"Programs\ChatGPT\ChatGPT.exe"], package: "chatgpt", publisher: "OpenAI" },
];

#[derive(Clone, Debug)]
pub enum Launch { Exe(PathBuf), Package(String /* AppUserModelID */) }

#[derive(Clone, Debug)]
pub struct Install { pub name: String, pub dir: PathBuf, pub launch: Launch }

/// A process whose exe lives in an install's folder.
pub struct RawProc { pub pid: u32, pub ppid: u32, pub app: usize, pub cmdline: String }
pub struct Running { pub pid: u32, pub app: usize, pub data_dir: Option<String> }

/// The apps' main processes: not Electron helpers (--type=…), and not started by another process of the
/// same apps (that skips Node child processes, e.g. MCP servers run through the app's own exe).
pub fn main_processes(raw: &[RawProc]) -> Vec<Running> {
    raw.iter()
        .filter(|p| !raw.iter().any(|q| q.pid == p.ppid && q.pid != p.pid))
        .filter_map(|p| match parse_cmdline(&p.cmdline) { (false, dir) => Some(Running { pid: p.pid, app: p.app, data_dir: dir }), _ => None })
        .collect()
}

/// Running processes for an item: a profile (by data folder) or a default app (no --user-data-dir).
pub fn pids_for(running: &[Running], app: usize, data_dir: Option<&Path>) -> Vec<u32> {
    running.iter().filter(|r| r.app == app && match (&r.data_dir, data_dir) {
        (None, None) => true,
        (Some(a), Some(b)) => same_path(a, b),
        _ => false,
    }).map(|r| r.pid).collect()
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Store {
    pub profiles: Vec<Profile>,
    pub favorites: Vec<String>, // ordered item keys
    pub collapsed: Vec<String>, // section names
    pub all_includes_defaults: bool,
    pub favorites_in_lists: bool,
}

impl Default for Store {
    fn default() -> Self { Store { profiles: vec![], favorites: vec![], collapsed: vec![], all_includes_defaults: false, favorites_in_lists: false } }
}

impl Store {
    pub fn load() -> Store {
        std::fs::read(store_path()).ok().and_then(|d| serde_json::from_slice(&d).ok()).unwrap_or_default()
    }
    pub fn save(&self) -> Result<(), String> {
        let path = store_path();
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self).unwrap()).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
    }
    pub fn profile(&self, id: &str) -> Option<&Profile> { self.profiles.iter().find(|p| p.id == id) }
    pub fn is_favorite(&self, key: &str) -> bool { self.favorites.iter().any(|k| k == key) }
    pub fn toggle_favorite(&mut self, key: &str) {
        if self.is_favorite(key) { self.favorites.retain(|k| k != key) } else { self.favorites.push(key.into()) }
    }
    pub fn unique_id(&self, source: &str, name: &str) -> String {
        let base = slug(&format!("{source}-{name}"));
        (1..).map(|i| if i == 1 { base.clone() } else { format!("{base}-{i}") }).find(|id| self.profile(id).is_none()).unwrap()
    }
}

/// ASCII-only, so it's safe in AppUserModelIDs and file names.
pub fn slug(s: &str) -> String { s.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect() }

/// Profile names become file names (shortcut, data folder).
pub fn clean_name(s: &str) -> Result<String, String> {
    let n = s.trim();
    if n.is_empty() { return Err("Enter a name.".into()) }
    if let Some(c) = n.chars().find(|c| r#"\/:*?"<>|"#.contains(*c) || c.is_control()) {
        return Err(format!("Names can't contain “{c}”."));
    }
    if n.ends_with('.') { return Err("Names can't end with a period.".into()) }
    if n.chars().count() > 60 { return Err("That name is too long.".into()) }
    Ok(n.into())
}

// MARK: Paths. Off Windows (dev builds only) everything goes to a temp folder.

fn env_dir(var: &str) -> PathBuf {
    if cfg!(windows) { std::env::var_os(var).map(PathBuf::from).expect(var) } else { std::env::temp_dir().join("aigent-dev").join(var) }
}
pub fn roaming() -> PathBuf { env_dir("APPDATA") }
pub fn local() -> PathBuf { env_dir("LOCALAPPDATA") }
pub fn store_path() -> PathBuf { roaming().join("AIGENT").join("aigent.json") }
pub fn icons_dir() -> PathBuf { local().join("AIGENT").join("Icons") }
pub fn start_menu_dir() -> PathBuf { roaming().join(r"Microsoft\Windows\Start Menu\Programs\AI Profiles") }
/// Local, not roaming (Chromium data is big), and named like the macOS app's: …\Profiles\Claude-Work.
pub fn default_data_dir(source: &str, name: &str) -> PathBuf { local().join("AIGENT").join("Profiles").join(format!("{source}-{name}")) }

// MARK: Command lines

/// Split a Windows command line the way CommandLineToArgvW does (for everything after argv[0]).
pub fn split_args(cmd: &str) -> Vec<String> {
    let (mut out, mut cur, mut quoted, mut any) = (vec![], String::new(), false, false);
    let chars: Vec<char> = cmd.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' {
            let n = chars[i..].iter().take_while(|&&c| c == '\\').count();
            if chars.get(i + n) == Some(&'"') {
                cur.extend(std::iter::repeat('\\').take(n / 2));
                if n % 2 == 1 { cur.push('"'); i += n + 1; } else { i += n; }
            } else {
                cur.extend(std::iter::repeat('\\').take(n));
                i += n;
            }
            any = true;
            continue;
        }
        match c {
            '"' if quoted && chars.get(i + 1) == Some(&'"') => { cur.push('"'); i += 1; }
            '"' => { quoted = !quoted; any = true; }
            ' ' | '\t' if !quoted => { if any { out.push(std::mem::take(&mut cur)); any = false; } }
            _ => { cur.push(c); any = true; }
        }
        i += 1;
    }
    if any { out.push(cur) }
    out
}

/// What an Electron process was started with: is it a helper (--type=…), and which data folder.
pub fn parse_cmdline(cmd: &str) -> (bool, Option<String>) {
    let args = split_args(cmd);
    (args.iter().any(|a| a.starts_with("--type=")), args.iter().find_map(|a| a.strip_prefix("--user-data-dir=").map(String::from)))
}

pub fn same_path(a: &str, b: &Path) -> bool {
    let norm = |s: &str| s.trim_matches('"').replace('/', "\\").trim_end_matches('\\').to_lowercase();
    norm(a) == norm(&b.to_string_lossy())
}

pub fn self_test() {
    assert_eq!(Look::defaults_for("Personal"), Look::defaults_for("Personal"), "stable defaults");
    assert!(Look::defaults_for("2 Dev").badge == "2" && Look::defaults_for("").badge == "?", "badge letter");
    assert_eq!(Look::defaults_for("Work").color, "#30D158", "same color as the macOS app");
    assert_eq!(slug("Archive - Development"), "archive---development", "slug");
    assert_eq!(graphemes("🦊") + graphemes("⭐️") + graphemes("👩‍💻"), 3, "emoji are one character");
    assert_eq!(last_graphemes("AB🦊", 2), "B🦊", "typing past the limit keeps the newest");
    let l = Look { badge: "🦊".into(), color: "#FF375F".into(), image_path: Some("C:\\a.png".into()), glyph: true, framed: true };
    assert_eq!(serde_json::from_str::<Look>(&serde_json::to_string(&l).unwrap()).unwrap(), l, "look round-trip");
    assert_eq!(serde_json::from_str::<Store>("{}").unwrap().profiles.len(), 0, "empty store");
    assert!(clean_name(" Work ").unwrap() == "Work" && clean_name("a/b").is_err() && clean_name("x.").is_err() && clean_name("  ").is_err(), "names");
    assert_eq!(split_args(r#"--a "b c" d\\"e f" "g\"h" """#), ["--a", "b c", "d\\e f", "g\"h", ""], "argv rules");
    let (helper, dir) = parse_cmdline(r#""--user-data-dir=C:\Users\a b\AppData\Roaming\Claude-Work" --flag"#);
    assert!(!helper && same_path(&dir.unwrap(), Path::new(r"c:\users\A B\appdata\roaming\claude-work\")), "data dir arg");
    assert!(parse_cmdline("--type=renderer --user-data-dir=C:\\x").0, "helper process");
    let mut s = Store::default();
    s.profiles.push(Profile { id: "claude-work".into(), name: "Work".into(), source: "Claude".into(), data_dir: "x".into(), look: Look::default() });
    assert_eq!(s.unique_id("Claude", "Work"), "claude-work-2", "unique id");
    s.toggle_favorite("app:Claude"); s.toggle_favorite("profile:claude-work"); s.toggle_favorite("app:Claude");
    assert_eq!(s.favorites, ["profile:claude-work"], "favorites toggle");
    let raw = |pid, ppid, cmd: &str| RawProc { pid, ppid, app: 0, cmdline: cmd.into() };
    let procs = [
        raw(10, 1, r#""C:\a\claude.exe""#),                                  // default app
        raw(11, 10, r#""C:\a\claude.exe" --type=renderer"#),                 // its helper
        raw(12, 10, r#""C:\a\claude.exe" C:\mcp\server.js"#),              // Node child of the default app
        raw(20, 1, r#""C:\a\claude.exe" "--user-data-dir=C:\p\Claude-Work""#), // profile
    ];
    let running = main_processes(&procs);
    assert_eq!(pids_for(&running, 0, None), [10], "default app main process only");
    assert_eq!(pids_for(&running, 0, Some(Path::new(r"C:\P\claude-work"))), [20], "profile main process");
    assert!(pids_for(&running, 1, None).is_empty(), "other app");
}
