// AIGENT for Windows: run several signed-in copies ("profiles") of Electron AI apps side by side.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod icon;
mod ui;
#[cfg(windows)]
mod win;
#[cfg(not(windows))]
#[path = "dev_stub.rs"]
mod win;

use crate::core::*;
use std::path::Path;
use std::time::{Duration, Instant};

// MARK: Profile operations (shared by the UI and the command line)

pub fn install_for(name: &str) -> Result<Install, String> {
    win::installed().into_iter().find(|i| i.name == name).ok_or_else(|| format!("{name} isn't installed."))
}

/// Main processes of every installed app, for running checks.
pub fn running(installs: &[Install]) -> Vec<Running> { main_processes(&win::processes(installs)) }

/// Write the profile's icon and Start Menu shortcut. Safe to re-run.
pub fn rebuild(p: &Profile) -> Result<(), String> {
    let base = install_for(&p.source).ok().and_then(|i| win::app_icon(&i));
    let ico = p.icon_path();
    icon::write_ico(&ico, |s| icon::render_icon(base.as_ref(), &p.look, s))?;
    // Earlier icons of this profile: <id>-<8 hex>.ico
    for e in std::fs::read_dir(icons_dir()).into_iter().flatten().flatten() {
        let path = e.path();
        let old = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.strip_prefix(&format!("{}-", p.id)))
            .is_some_and(|h| h.len() == 8 && h.chars().all(|c| c.is_ascii_hexdigit()));
        if old && path.extension().is_some_and(|x| x == "ico") && path != ico { let _ = std::fs::remove_file(path); }
    }
    win::write_shortcut(&p.shortcut_path(), &format!("--launch {}", p.id), &ico, &p.aumid(), &p.display())?;
    win::icons_changed();
    Ok(())
}

pub fn add_profile(source: &str, name: &str) -> Result<Profile, String> {
    let name = clean_name(name)?;
    let mut s = Store::load();
    if s.profiles.iter().any(|p| p.source == source && p.name.to_lowercase() == name.to_lowercase()) {
        return Err(format!("There's already a {source} profile named “{name}”."));
    }
    let p = Profile { id: s.unique_id(source, &name), data_dir: default_data_dir(source, &name), look: Look::defaults_for(&name), name, source: source.into() };
    std::fs::create_dir_all(&p.data_dir).map_err(|e| format!("Couldn't create the data folder: {e}"))?;
    rebuild(&p)?;
    s.profiles.push(p.clone());
    s.save()?;
    Ok(p)
}

/// Rename and/or restyle. Applies right away, even while it's running (the watcher picks up the new icon).
pub fn edit_profile(id: &str, name: &str, look: Look) -> Result<(), String> {
    let name = clean_name(name)?;
    let mut s = Store::load();
    let p = s.profiles.iter_mut().find(|p| p.id == id).ok_or("That profile no longer exists.")?;
    let old_shortcut = p.shortcut_path();
    p.name = name;
    p.look = look;
    let p = p.clone();
    if old_shortcut != p.shortcut_path() { let _ = std::fs::remove_file(old_shortcut); }
    rebuild(&p)?;
    s.save()
}

/// Removes the shortcut and icon. The data folder (sign-in, sessions) stays, so re-adding the name restores it.
pub fn delete_profile(id: &str) -> Result<(), String> {
    let mut s = Store::load();
    let Some(p) = s.profile(id).cloned() else { return Ok(()) };
    let _ = std::fs::remove_file(p.shortcut_path());
    let _ = std::fs::remove_file(p.icon_path());
    s.profiles.retain(|q| q.id != id);
    s.favorites.retain(|k| *k != p.key());
    s.save()
}

/// Start a profile through `AIGENT --launch`, which keeps watching it to give its windows their own taskbar button.
pub fn launch_profile(id: &str) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::process::Command::new(exe).args(["--launch", id]).spawn().map(|_| ()).map_err(|e| e.to_string())
}

fn launch_and_watch(id: &str) -> Result<(), String> {
    let p = Store::load().profile(id).cloned().ok_or("That profile no longer exists.")?;
    let inst = install_for(&p.source)?;
    if !p.icon_path().exists() || !p.shortcut_path().exists() { rebuild(&p)?; }
    std::fs::create_dir_all(&p.data_dir).map_err(|e| e.to_string())?;
    win::start(&inst, Some(&p.data_dir))?;
    let Some(_lock) = win::single_instance(&format!("AIGENT-watch-{id}")) else { return Ok(()) }; // already watched
    let relaunch = format!("\"{}\" --launch {id}", std::env::current_exe().map_err(|e| e.to_string())?.display());
    let mut stamper = win::Stamper::default();
    let (started, mut last_seen) = (Instant::now(), None::<Instant>);
    loop {
        std::thread::sleep(Duration::from_millis(700));
        let Some(p) = Store::load().profile(id).cloned() else { return Ok(()) }; // deleted
        let pids = pids_for(&running(std::slice::from_ref(&inst)), 0, Some(&p.data_dir));
        if pids.is_empty() {
            // Give it time to start, and to come back if it restarts itself.
            let gone = last_seen.map_or(started.elapsed() > Duration::from_secs(60), |t| t.elapsed() > Duration::from_secs(10));
            if gone { return Ok(()) }
            continue;
        }
        last_seen = Some(Instant::now());
        stamper.stamp(&pids, &p.aumid(), &relaunch, &p.display(), &p.icon_path());
    }
}

/// Quit (normally, then forcibly if it won't) and start again.
pub fn restart(key: &str) -> Result<(), String> {
    let installs = win::installed();
    let s = Store::load();
    let profile = key.strip_prefix("profile:").and_then(|id| s.profile(id)).cloned();
    let source = profile.as_ref().map(|p| p.source.clone()).or_else(|| key.strip_prefix("app:").map(String::from)).ok_or("Unknown item.")?;
    let app = installs.iter().position(|i| i.name == source).ok_or_else(|| format!("{source} isn't installed."))?;
    win::quit(&pids_for(&running(&installs), app, profile.as_ref().map(|p| p.data_dir.as_path())), false);
    match profile { Some(p) => launch_profile(&p.id), None => win::start(&installs[app], None) }
}

// MARK: Command line

fn cli(job: impl FnOnce() -> Result<(), String>) -> ! {
    match job() {
        Ok(()) => std::process::exit(0),
        Err(e) => { eprintln!("{e}"); std::process::exit(1) }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |flag: &str, n: usize| args.iter().position(|a| a == flag).filter(|i| i + n < args.len()).map(|i| args[i + 1..=i + n].to_vec());
    if args.len() > 1 && !args.iter().any(|a| a == "--launch") { win::attach_console(); }

    if let Some(a) = arg("--launch", 1) { cli(|| launch_and_watch(&a[0])) }
    if let Some(a) = arg("--add", 2) { cli(|| add_profile(&a[0], &a[1]).map(|p| println!("{}", p.id))) }
    if let Some(a) = arg("--rename", 2) {
        cli(|| { let look = Store::load().profile(&a[0]).ok_or("No such profile.")?.look.clone(); edit_profile(&a[0], &a[1], look) })
    }
    if let Some(a) = arg("--restart", 1) { cli(|| restart(&a[0])) }
    if let Some(a) = arg("--write-app-icon", 1) { cli(|| icon::write_ico(Path::new(&a[0]), icon::render_app_icon)) }
    if args.iter().any(|a| a == "--self-test") {
        cli(|| {
            core::self_test();
            icon::self_test();
            #[cfg(windows)]
            {
                let lnk = std::env::temp_dir().join("aigent-self-test.lnk");
                win::write_shortcut(&lnk, "--launch x", &std::env::temp_dir().join("x.ico"), "AIGENT.Profile.self-test", "test")?;
                let id = win::shortcut_aumid(&lnk);
                let _ = std::fs::remove_file(&lnk);
                match id.as_deref() {
                    Err(e) if e.contains("Not implemented") => println!("shortcut AppUserModelID: can't be read back here (Wine), skipped"),
                    id => assert_eq!(id, Ok("AIGENT.Profile.self-test"), "shortcut AppUserModelID"),
                }
                println!("Found: {:?}", win::installed().iter().map(|i| (&i.name, &i.launch)).collect::<Vec<_>>());
            }
            println!("self-test passed");
            Ok(())
        })
    }

    // One window: a second start just brings the first one forward.
    let Some(_lock) = win::single_instance("AIGENT-gui") else { win::focus_window_titled("AIGENT"); return };
    // Shortcuts point at this exe; recreate any that are missing (e.g. after reinstalling somewhere else).
    for p in Store::load().profiles { if !p.shortcut_path().exists() || !p.icon_path().exists() { let _ = rebuild(&p); } }
    ui::run();
}
