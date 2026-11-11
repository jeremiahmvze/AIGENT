# AIGENT (beta)

A small macOS app for running several separately signed-in copies of **Claude** and **ChatGPT**
desktop at the same time. Each copy ("profile") is a real app with its own Dock icon, name,
windows and sign-in. No overlays or borders, and no extra Dock icons.

## How it works
A profile is an APFS clone of the installed app (it uses almost no disk space). It has its own
bundle ID and icon, and starts with `--user-data-dir` pointing at its own data folder. Profiles
live in `~/Applications/AI Profiles/` and run without the launcher being open. When Claude or
ChatGPT updates, a profile rebuilds itself on its next launch. Sign-ins and data are kept.

Features: Launch All / Close All, Favorites (drag to reorder), collapsible sections, custom
profile icons (badge, color, image), and Restart / Update buttons on each row.

## Install
Requires macOS 14+ on Apple Silicon, plus Claude and/or ChatGPT in `/Applications`.

1. Download `AIGENT-*.dmg` from [Releases](../../releases), open it, and drag **AIGENT** onto
   **Applications**. It must live there, because profiles use it to update.
2. The beta isn't notarized, so the first time you open it, macOS blocks it. To allow it, go to
   **System Settings → Privacy & Security** and click **Open Anyway**.

## Build from source
    swift build && .build/debug/AILauncher --self-test   # quick check
    ./build.sh                                           # build, sign, install to /Applications
    ./build.sh --release                                 # ad-hoc signed .dmg in build/

### Keeping permissions across rebuilds (optional)
Profiles are ad-hoc signed by default, so macOS can ask for Keychain/privacy approvals again after
each rebuild. To avoid that, create a self-signed **Code Signing** certificate named
`AI Launcher Local` in Keychain Access (Certificate Assistant → Create a Certificate). Profiles
and local builds use it automatically. Don't use that certificate for builds you share, because
it embeds the details you entered.

## Notes
- This is an unofficial tool, not affiliated with Anthropic or OpenAI.
- Deleting a profile moves the app to the Trash but keeps its data folder in
  `~/Library/Application Support/`.

MIT licensed.
