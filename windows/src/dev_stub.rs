// Stand-in for win.rs so the UI and self-test run on macOS/Linux during development. Not shipped.
use crate::core::{Install, Launch, RawProc};
use std::path::{Path, PathBuf};
use tiny_skia::Pixmap;

pub fn attach_console() {}
pub fn installed() -> Vec<Install> {
    vec![Install { name: "Claude".into(), dir: PathBuf::from("/nonexistent"), launch: Launch::Exe("/nonexistent/claude".into()) }]
}
pub fn app_icon(_: &Install) -> Option<Pixmap> { None }
pub fn processes(_: &[Install]) -> Vec<RawProc> { vec![] }
pub fn start(i: &Install, _: Option<&Path>) -> Result<(), String> { Err(format!("{} can only be started on Windows.", i.name)) }
pub fn focus(_: &[u32]) -> bool { false }
pub fn quit(_: &[u32], _: bool) {}
#[derive(Default)]
pub struct Stamper;
impl Stamper { pub fn stamp(&mut self, _: &[u32], _: &str, _: &str, _: &str, _: &Path) {} }
pub fn single_instance(_: &str) -> Option<()> { Some(()) }
pub fn focus_window_titled(_: &str) {}
pub fn write_shortcut(lnk: &Path, args: &str, _: &Path, _: &str, _: &str) -> Result<(), String> {
    std::fs::create_dir_all(lnk.parent().unwrap()).and_then(|_| std::fs::write(lnk, args)).map_err(|e| e.to_string())
}
pub fn icons_changed() {}
pub fn open_at_login() -> bool { false }
pub fn set_open_at_login(_: bool) -> Result<(), String> { Err("Windows only.".into()) }
pub fn emoji_panel() {}
pub fn show_folder(_: &Path) {}
