// Windows integration: finding installs, processes, launching, taskbar identity, shortcuts, login item.
use crate::core::{Install, Launch, RawProc, KNOWN};
use std::collections::HashMap;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tiny_skia::Pixmap;
use windows::core::{Interface, BOOL, HSTRING, PCWSTR, PWSTR};
use windows::Management::Deployment::PackageManager;
use windows::Wdk::System::Threading::{NtQueryInformationProcess, PROCESSINFOCLASS};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::Storage::EnhancedStorage::*;
use windows::Win32::System::Com::StructuredStorage::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
use windows::Win32::System::Diagnostics::ToolHelp::*;
use windows::Win32::System::Registry::*;
use windows::Win32::System::Threading::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::PropertiesSystem::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

fn com() { unsafe { let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED); } }
fn err(e: windows::core::Error) -> String { e.message() }

/// So CLI output (e.g. --self-test) shows up when run from a terminal; the GUI has no console.
pub fn attach_console() { unsafe { let _ = AttachConsole(ATTACH_PARENT_PROCESS); } }

// MARK: Installs

pub fn installed() -> Vec<Install> {
    let local = crate::core::local();
    let mut packages = None;
    KNOWN.iter().filter_map(|k| {
        let exe = k.exes.iter().map(|e| local.join(e)).find(|p| p.is_file());
        if let Some(exe) = exe { return Some(Install { name: k.name.into(), dir: exe.parent().unwrap().into(), launch: Launch::Exe(exe) }) }
        packages.get_or_insert_with(find_packages).iter()
            .find(|(name, publisher, ..)| name.contains(k.package) && publisher.contains(k.publisher))
            .map(|(_, _, dir, aumid)| Install { name: k.name.into(), dir: dir.clone(), launch: Launch::Package(aumid.clone()) })
    }).collect()
}

/// (lowercased package name, publisher, install folder, AppUserModelID) of MSIX/Store packages that might be known apps.
fn find_packages() -> Vec<(String, String, PathBuf, String)> {
    com();
    let Ok(list) = PackageManager::new().and_then(|m| m.FindPackagesByUserSecurityId(&HSTRING::new())) else { return vec![] };
    list.into_iter().filter_map(|p| {
        let name = p.Id().ok()?.Name().ok()?.to_string().to_lowercase();
        if !KNOWN.iter().any(|k| name.contains(k.package)) || p.IsFramework().unwrap_or(true) { return None }
        let aumid = p.GetAppListEntries().ok()?.GetAt(0).ok()?.AppUserModelId().ok()?.to_string();
        Some((name, p.PublisherDisplayName().ok()?.to_string(), PathBuf::from(p.InstalledPath().ok()?.to_string()), aumid))
    }).collect()
}

/// The app's own 256 px icon, from the shell (works for exes and Store apps alike).
pub fn app_icon(i: &Install) -> Option<Pixmap> {
    com();
    let path = match &i.launch { Launch::Exe(p) => p.display().to_string(), Launch::Package(a) => format!(r"shell:AppsFolder\{a}") };
    unsafe {
        let f: IShellItemImageFactory = SHCreateItemFromParsingName(&HSTRING::from(path), None).ok()?;
        let hbm = f.GetImage(SIZE { cx: 256, cy: 256 }, SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK).ok()?;
        let px = bitmap_rgba(hbm);
        let _ = DeleteObject(hbm.into());
        px
    }
}

unsafe fn bitmap_rgba(hbm: HBITMAP) -> Option<Pixmap> {
    let mut bm = BITMAP::default();
    if GetObjectW(hbm.into(), size_of::<BITMAP>() as i32, Some(&mut bm as *mut _ as _)) == 0 { return None }
    let (w, h) = (bm.bmWidth, bm.bmHeight.abs());
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER { biSize: size_of::<BITMAPINFOHEADER>() as u32, biWidth: w, biHeight: -h, biPlanes: 1, biBitCount: 32, ..Default::default() },
        ..Default::default()
    };
    let mut buf = vec![0u8; (w * h * 4) as usize];
    let dc = GetDC(None);
    let rows = GetDIBits(dc, hbm, 0, h as u32, Some(buf.as_mut_ptr() as _), &mut info, DIB_RGB_COLORS);
    ReleaseDC(None, dc);
    if rows == 0 { return None }
    for p in buf.chunks_exact_mut(4) { p.swap(0, 2) } // BGRA → RGBA (premultiplied)
    if buf.chunks_exact(4).all(|p| p[3] == 0) { for p in buf.chunks_exact_mut(4) { p[3] = 255 } } // icon without alpha
    crate::icon::from_rgba(w as u32, h as u32, buf, true)
}

// MARK: Processes

/// Processes whose exe is inside one of the installs' folders, with their command lines.
pub fn processes(installs: &[Install]) -> Vec<RawProc> {
    let mut out = vec![];
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return out };
        let mut e = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(snap, &mut e).is_ok();
        while ok {
            let (pid, ppid) = (e.th32ProcessID, e.th32ParentProcessID);
            if let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                if let Some(app) = image(h).and_then(|img| installs.iter().position(|i| img.starts_with(&i.dir))) {
                    if let Some(cmdline) = cmdline(h) { out.push(RawProc { pid, ppid, app, cmdline }) }
                }
                let _ = CloseHandle(h);
            }
            ok = Process32NextW(snap, &mut e).is_ok();
        }
        let _ = CloseHandle(snap);
    }
    out
}

unsafe fn image(h: HANDLE) -> Option<PathBuf> {
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).ok()?;
    Some(PathBuf::from(String::from_utf16_lossy(&buf[..len as usize])))
}

unsafe fn cmdline(h: HANDLE) -> Option<String> {
    const PROCESS_COMMAND_LINE_INFORMATION: PROCESSINFOCLASS = PROCESSINFOCLASS(60); // Windows 8.1+
    let mut len = 0u32;
    let _ = NtQueryInformationProcess(h, PROCESS_COMMAND_LINE_INFORMATION, std::ptr::null_mut(), 0, &mut len);
    if len == 0 { return None }
    let mut buf = vec![0u64; (len as usize).div_ceil(8)]; // u64 for UNICODE_STRING alignment
    NtQueryInformationProcess(h, PROCESS_COMMAND_LINE_INFORMATION, buf.as_mut_ptr() as _, len, &mut len).ok().ok()?;
    let us = &*(buf.as_ptr() as *const windows::Win32::Foundation::UNICODE_STRING);
    Some(String::from_utf16_lossy(std::slice::from_raw_parts(us.Buffer.0, us.Length as usize / 2)))
}

/// Start the app, with its own data folder for a profile.
pub fn start(i: &Install, data_dir: Option<&Path>) -> Result<(), String> {
    let arg = data_dir.map(|d| format!("--user-data-dir={}", d.display()));
    match &i.launch {
        Launch::Exe(exe) => {
            let mut c = Command::new(exe);
            c.args(&arg).current_dir(exe.parent().unwrap()).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
            c.creation_flags((DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP).0);
            // Optional isolated Claude Code config, like the macOS app.
            if let Some(cfg) = data_dir.map(|d| d.join("claude-config")).filter(|d| d.is_dir()) { c.env("CLAUDE_CONFIG_DIR", cfg); }
            c.spawn().map(|_| ()).map_err(|e| format!("Couldn't start {}: {e}", i.name))
        }
        Launch::Package(aumid) => unsafe {
            com();
            let m: IApplicationActivationManager = CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER).map_err(err)?;
            let args = arg.map(|a| format!("\"{a}\"")).unwrap_or_default();
            m.ActivateApplication(&HSTRING::from(aumid.as_str()), &HSTRING::from(args), AO_NONE).map(|_| ()).map_err(|e| format!("Couldn't start {}: {}", i.name, err(e)))
        },
    }
}

/// Top-level, unowned windows of these processes.
fn windows_of(pids: &[u32], visible_only: bool) -> Vec<HWND> {
    unsafe extern "system" fn each(h: HWND, l: LPARAM) -> BOOL {
        (*(l.0 as *mut Vec<HWND>)).push(h);
        true.into()
    }
    let mut all: Vec<HWND> = vec![];
    unsafe { let _ = EnumWindows(Some(each), LPARAM(&mut all as *mut _ as isize)); }
    all.into_iter().filter(|&h| unsafe {
        let mut pid = 0;
        GetWindowThreadProcessId(h, Some(&mut pid));
        pids.contains(&pid) && GetWindow(h, GW_OWNER).map_or(true, |o| o.is_invalid()) && (!visible_only || IsWindowVisible(h).as_bool())
    }).collect()
}

/// Bring the app's window forward. False if it has none showing (e.g. it's in the tray).
pub fn focus(pids: &[u32]) -> bool {
    let Some(&h) = windows_of(pids, true).first() else { return false };
    unsafe {
        if IsIconic(h).as_bool() { let _ = ShowWindow(h, SW_RESTORE); }
        SetForegroundWindow(h).as_bool()
    }
}

/// Ask the windows to close, then end whatever is left. Force: end it right away.
pub fn quit(pids: &[u32], force: bool) {
    if !force {
        for h in windows_of(pids, true) { unsafe { let _ = PostMessageW(Some(h), WM_CLOSE, WPARAM(0), LPARAM(0)); } }
        // ponytail: Electron apps that hide to the tray on close only quit when ended; Chromium's storage survives that.
        let deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < deadline && pids.iter().any(|&p| alive(p)) { std::thread::sleep(Duration::from_millis(100)) }
    }
    for &pid in pids {
        unsafe {
            if let Ok(h) = OpenProcess(PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, false, pid) {
                let _ = TerminateProcess(h, 0);
                WaitForSingleObject(h, 5000);
                let _ = CloseHandle(h);
            }
        }
    }
}

fn alive(pid: u32) -> bool {
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) else { return false };
        let running = WaitForSingleObject(h, 0) == WAIT_TIMEOUT;
        let _ = CloseHandle(h);
        running
    }
}

// MARK: Taskbar identity

/// A VT_LPWSTR value pointing at our own buffer (SetValue copies it, so no PropVariantClear).
unsafe fn set_prop(store: &IPropertyStore, key: &PROPERTYKEY, value: &str) -> windows::core::Result<()> {
    let mut wide: Vec<u16> = value.encode_utf16().chain([0]).collect();
    let mut v = PROPVARIANT::default();
    (*v.Anonymous.Anonymous).vt = windows::Win32::System::Variant::VT_LPWSTR;
    (*v.Anonymous.Anonymous).Anonymous.pwszVal = PWSTR(wide.as_mut_ptr());
    store.SetValue(key, &v)
}

/// Gives a profile's windows their own taskbar button: its AppUserModelID (same as its Start Menu shortcut,
/// so pins match), relaunch command, name and icon.
#[derive(Default)]
pub struct Stamper { icons: HashMap<PathBuf, (HICON, HICON)>, done: HashMap<isize, PathBuf> }

impl Stamper {
    pub fn stamp(&mut self, pids: &[u32], aumid: &str, relaunch: &str, name: &str, icon: &Path) {
        com();
        let (big, small) = *self.icons.entry(icon.into()).or_insert_with(|| unsafe {
            let load = |s| LoadImageW(None, &HSTRING::from(icon), IMAGE_ICON, s, s, LR_LOADFROMFILE).map(|h| HICON(h.0)).unwrap_or_default();
            (load(GetSystemMetrics(SM_CXICON)), load(GetSystemMetrics(SM_CXSMICON)))
        });
        for h in windows_of(pids, false) {
            if self.done.get(&(h.0 as isize)).is_some_and(|p| p == icon) { continue }
            unsafe {
                if let Ok(store) = SHGetPropertyStoreForWindow::<IPropertyStore>(h) {
                    // Relaunch details first; the ID last, since setting it is what moves the window to its own button.
                    let _ = set_prop(&store, &PKEY_AppUserModel_RelaunchCommand, relaunch);
                    let _ = set_prop(&store, &PKEY_AppUserModel_RelaunchDisplayNameResource, name);
                    let _ = set_prop(&store, &PKEY_AppUserModel_RelaunchIconResource, &format!("{},0", icon.display()));
                    let _ = set_prop(&store, &PKEY_AppUserModel_ID, aumid);
                }
                for (kind, hi) in [(ICON_BIG, big), (ICON_SMALL, small)] {
                    if !hi.is_invalid() {
                        SendMessageTimeoutW(h, WM_SETICON, WPARAM(kind as usize), LPARAM(hi.0 as isize), SMTO_ABORTIFHUNG, 500, None);
                    }
                }
            }
            self.done.insert(h.0 as isize, icon.into());
        }
    }
}

/// Held while a watcher runs, so a second launch of the same profile doesn't start another.
pub fn single_instance(name: &str) -> Option<HANDLE> {
    unsafe {
        let h = CreateMutexW(None, true, &HSTRING::from(format!(r"Local\{name}"))).ok()?;
        if GetLastError() == ERROR_ALREADY_EXISTS { let _ = CloseHandle(h); None } else { Some(h) }
    }
}

/// Bring an already-open AIGENT window forward.
pub fn focus_window_titled(title: &str) {
    unsafe {
        if let Ok(h) = FindWindowW(PCWSTR::null(), &HSTRING::from(title)) {
            let _ = ShowWindow(h, SW_RESTORE);
            let _ = SetForegroundWindow(h);
        }
    }
}

// MARK: Shortcuts

pub fn write_shortcut(lnk: &Path, args: &str, icon: &Path, aumid: &str, description: &str) -> Result<(), String> {
    com();
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(lnk.parent().unwrap()).map_err(|e| e.to_string())?;
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(err)?;
        link.SetPath(&HSTRING::from(exe.as_path())).map_err(err)?;
        link.SetArguments(&HSTRING::from(args)).map_err(err)?;
        link.SetWorkingDirectory(&HSTRING::from(exe.parent().unwrap())).map_err(err)?;
        link.SetIconLocation(&HSTRING::from(icon), 0).map_err(err)?;
        link.SetDescription(&HSTRING::from(description)).map_err(err)?;
        let store: IPropertyStore = link.cast().map_err(err)?;
        set_prop(&store, &PKEY_AppUserModel_ID, aumid).map_err(err)?;
        store.Commit().map_err(err)?;
        link.cast::<IPersistFile>().map_err(err)?.Save(&HSTRING::from(lnk), true).map_err(|e| format!("Couldn't save the shortcut: {}", err(e)))
    }
}

pub fn shortcut_aumid(lnk: &Path) -> Result<String, String> {
    com();
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(err)?;
        link.cast::<IPersistFile>().map_err(err)?.Load(&HSTRING::from(lnk), STGM_READ).map_err(|e| format!("load: {}", err(e)))?;
        let mut v = link.cast::<IPropertyStore>().map_err(err)?.GetValue(&PKEY_AppUserModel_ID).map_err(|e| format!("get: {}", err(e)))?;
        let s = PropVariantToStringAlloc(&v).map_err(|e| format!("to string: {}", err(e)))?;
        let out = s.to_string().map_err(|e| e.to_string());
        CoTaskMemFree(Some(s.0 as _));
        let _ = PropVariantClear(&mut v);
        out
    }
}

/// Tell Explorer icons changed (Start Menu, pinned items).
pub fn icons_changed() { unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) } }

// MARK: Misc

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

pub fn open_at_login() -> bool {
    unsafe { RegGetValueW(HKEY_CURRENT_USER, &HSTRING::from(RUN_KEY), &HSTRING::from("AIGENT"), RRF_RT_REG_SZ, None, None, None).is_ok() }
}

pub fn set_open_at_login(on: bool) -> Result<(), String> {
    let (key, name) = (HSTRING::from(RUN_KEY), HSTRING::from("AIGENT"));
    let r = unsafe {
        if on {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let v: Vec<u16> = format!("\"{}\"", exe.display()).encode_utf16().chain([0]).collect();
            RegSetKeyValueW(HKEY_CURRENT_USER, &key, &name, REG_SZ.0, Some(v.as_ptr() as _), (v.len() * 2) as u32)
        } else {
            RegDeleteKeyValueW(HKEY_CURRENT_USER, &key, &name)
        }
    };
    r.ok().map_err(|e| err(e))
}

/// Windows' emoji panel (Win + .) for the focused text field.
pub fn emoji_panel() {
    let key = |vk: VIRTUAL_KEY, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) }, ..Default::default() } },
    };
    let inputs = [key(VK_LWIN, false), key(VK_OEM_PERIOD, false), key(VK_OEM_PERIOD, true), key(VK_LWIN, true)];
    unsafe { SendInput(&inputs, size_of::<INPUT>() as i32); }
}

pub fn show_folder(path: &Path) { let _ = Command::new("explorer").arg(path).spawn(); }
