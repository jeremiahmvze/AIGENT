# AIGENT (beta)

A small and completely vibe-coded macOS and Windows app for running several separately signed-in copies of **Claude** and **ChatGPT** (more OTW)
desktop at the same time. Each copy ("profile") is a real app with its own Dock icon, name,
windows and sign-in. No overlays or borders, and no extra Dock icons.

## How it works
A profile is an APFS clone of the installed app (it uses almost no disk space). It has its own
bundle ID and icon, and starts with `--user-data-dir` pointing at its own data folder. Profiles
live in `~/Applications/AI Profiles/` and run without the launcher being open. When Claude or
ChatGPT updates, a profile rebuilds itself on its next launch. Sign-ins and data are kept.

Features: Launch All / Close All, Favorites (drag to reorder), collapsible sections, custom
profile icons (app icon with a badge, an emoji or letter tile, or your own image), and Restart / Update buttons on each row.

## Install
Requires macOS 14+ on Apple Silicon, plus Claude and/or ChatGPT in `/Applications`.

1. Download `AIGENT-*.dmg` from [Releases](../../releases), open it, and drag **AIGENT** onto
   **Applications**. It must live there, because profiles use it to update.
2. The beta isn't notarized, so the first time you open it, macOS blocks it. To allow it, go to
   **System Settings → Privacy & Security** and click **Open Anyway**.

## Build from source
    swift build && .build/debug/AIGENT --self-test   # quick check
    ./build.sh                                           # build, sign, install to /Applications
    ./build.sh --release                                 # ad-hoc signed .dmg in build/

### Keeping permissions across rebuilds (optional)
Profiles are ad-hoc signed by default, so macOS can ask for Keychain/privacy approvals again after
each rebuild. To avoid that, create a self-signed **Code Signing** certificate named
`AI Launcher Local` in Keychain Access (Certificate Assistant → Create a Certificate). Profiles
and local builds use it automatically. Don't use that certificate for builds you share, because
it embeds the details you entered.

## Windows
Requires Windows 10 or 11 (x64), plus Claude and/or ChatGPT for Windows.

1. Download `AIGENT-Setup-*.exe` from [Releases](../../releases) and run it. It installs for your
   account only (no admin prompt) to `%LOCALAPPDATA%\Programs\AIGENT`.
2. The installer isn't signed, so SmartScreen may warn. Click **More info → Run anyway**.

On Windows a profile isn't a copy of the app. It's a Start Menu shortcut (in **AI Profiles**) that
starts the installed app with its own `--user-data-dir`, plus its own icon and taskbar button.
The app's own updates apply to every profile, so there's nothing to rebuild. Uninstalling keeps
sign-ins and data (`%APPDATA%\AIGENT`, `%LOCALAPPDATA%\AIGENT`).

Windows caveats:
- Emoji badges are drawn in one colour (white).
- Store (MSIX) installs: Windows may redirect their data folders, and the Claude Code config folder
  (`CLAUDE_CONFIG_DIR`) isn't passed to them.
- After an app restarts itself to update, a profile can reopen as the default app. Launch it again
  from AIGENT.
- ChatGPT for Windows honouring `--user-data-dir` is unverified.

Build from source (on Windows, or on a Mac with `brew install mingw-w64 makensis`):

    cd windows
    cargo run -- --self-test   # quick check
    ./build.sh                 # build/windows/AIGENT.exe and build/AIGENT-Setup-<version>.exe

## Notes
- This is an unofficial tool, not affiliated with Anthropic or OpenAI.
- Deleting a profile moves the app to the Trash but keeps its data folder in
  `~/Library/Application Support/`.

MIT licensed.
