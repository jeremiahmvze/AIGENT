# Roadmap

## Done (1.1 beta)
- [x] Profiles: separate, signed-in copies of Claude and ChatGPT, each with its own Dock icon and name.
- [x] Launch All / Close All toolbar buttons (Close All asks first, then quits normally).
- [x] Collapsible sections (Favorites, Apps, Profiles); collapsed state is remembered.
- [x] Favorites at the top: right-click → Add to Favorites; drag to reorder.
- [x] Custom profile icons: name, 1–2 character/emoji badge, badge color, or a custom image.
- [x] Per-row Restart (running) and Update (source app updated) buttons.
- [x] Profiles rebuild themselves after the source app updates.
- [x] Settings: Open at Login, include default apps in Launch All / Close All.

## Ideas
- More Electron apps (Gemini, Perplexity, …) – add to `knownApps` in `Profiles.swift`.
- Universal (Intel + Apple Silicon) release build.
- Notarized release, so Gatekeeper doesn't warn.

## Known issues
- Profile icons use the macOS app-icon style active when they were built (e.g. Tinted/Clear).
  After changing the system icon style, Rebuild profiles to refresh them.
- Each profile launch leaves a harmless, invisible LaunchServices entry for its startup script.
