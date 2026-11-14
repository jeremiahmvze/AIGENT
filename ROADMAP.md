# Roadmap

## Done (1.1 beta)
- [x] Profiles: separate, signed-in copies of Claude and ChatGPT, each with its own Dock icon and name.
- [x] Launch All / Close All toolbar buttons (Close All asks first, then quits normally).
- [x] Collapsible sections (Favorites, Apps, Profiles); collapsed state is remembered.
- [x] Favorites at the top: right-click → Add to Favorites; drag to reorder.
- [x] Custom profile icons: an emoji or letter shown as a badge on the app icon or as the whole icon,
      picked from a Reminders-style emoji grid (or the full system picker); or a custom image.
- [x] Full Size or Framed icons.
- [x] Edits to a running profile are queued and applied when it restarts.
- [x] Icons stay inside the macOS icon shape, so macOS doesn't shrink them into a grey frame.
- [x] Per-row Restart (running) and Update (source app updated) buttons.
- [x] Profiles rebuild themselves after the source app updates.
- [x] Settings: Open at Login, include default apps in Launch All / Close All.

## Planned
### Import existing data
On first launch, and when adding the first profile for a service, ask whether to bring over data
from what's already installed (desktop sign-in, Claude Code tab sessions, local app state).
- Detect every existing data folder for that service: the default app's
  (`~/Library/Application Support/Claude`, ChatGPT/Codex folders) plus ones left by other tools
  (e.g. Multi-Claude's `<App>-<Name>` folders).
- If there's more than one, list them all with checkboxes; each one checked becomes its own profile.
- Copy with an APFS clone (near-zero disk) and leave the original untouched, so the default app keeps
  working. Ask the user to quit the source app first so the copy is consistent.
- Expect one Keychain prompt per imported profile (the copied sign-in is encrypted with the app's
  Keychain key); say so in the dialog.
- Also offer it later from the Add Profile row ("Import from…"), not only on first run.

### More services
- Perplexity first, then other popular desktop AI apps (e.g. Gemini if a desktop app exists).
- Check each app's build before adding it: Electron apps take `--user-data-dir` and drop straight
  into `knownApps` in `Profiles.swift`. Native (Swift) apps don't, so they need a different way to
  keep data separate, or can't be supported.
- Show only services that are installed (already how Claude/ChatGPT work).

## Ideas
- Universal (Intel + Apple Silicon) release build.
- Notarized release, so Gatekeeper doesn't warn.

## Fixed
- Profiles couldn't reach the local network ("No route to host", no Local Network prompt) because
  cloned binaries shared the original app's UUID. Each profile now gets its own (issue #1).

## Known issues
- Profile icons use the macOS app-icon style active when they were built (e.g. Tinted/Clear).
  After changing the system icon style, Rebuild profiles to refresh them.
- Each profile launch leaves a harmless, invisible LaunchServices entry for its startup script.
