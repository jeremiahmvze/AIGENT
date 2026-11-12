// Profile engine: each profile is an APFS clone of a real Electron app with its own bundle id,
// icon and --user-data-dir, so it gets its own Dock icon, windows and sign-in.
import AppKit

let fm = FileManager.default
let home = fm.homeDirectoryForCurrentUser
let profilesDir = home.appendingPathComponent("Applications/AI Profiles")
let supportDir = home.appendingPathComponent("Library/Application Support")
let iconsDir = supportDir.appendingPathComponent("AIGENT/Icons")
let launcherBinary = "/Applications/AIGENT.app/Contents/MacOS/AIGENT"
let signingCert = "AI Launcher Local"
// ponytail: hardcoded list; add an entry for any other Electron app (Gemini, Perplexity...).
let knownApps = ["Claude", "ChatGPT"]
let palette = ["#0A84FF", "#30D158", "#BF5AF2", "#FF375F", "#40C8E0", "#5E5CE6", "#FF9F0A", "#FF453A"]

/// How a profile's icon looks. Stored in the profile's Info.plist so rebuilds keep it.
struct Look: Equatable {
    var badge: String        // 0–2 characters; empty = no badge
    var color: String        // badge color, "#RRGGBB"
    var imagePath: String?   // custom base image instead of the app's icon
    var glyph = false        // true: the whole icon is `badge` (an emoji or letter) on a `color` tile

    static func defaults(for name: String) -> Look {
        let idx = name.unicodeScalars.reduce(0) { ($0 &* 31 &+ Int($1.value)) & 0xffff } % palette.count
        return Look(badge: String(name.first(where: { $0.isLetter || $0.isNumber }) ?? "?").uppercased(), color: palette[idx])
    }
}

struct Profile: Identifiable, Hashable {
    let url: URL
    var id: String { url.path }
    var info: [String: Any] { NSDictionary(contentsOf: url.appendingPathComponent("Contents/Info.plist")) as? [String: Any] ?? [:] }
    var name: String { info["AILName"] as? String ?? url.deletingPathExtension().lastPathComponent }
    var source: URL { URL(fileURLWithPath: info["AILSource"] as? String ?? "") }
    var dataDir: URL { URL(fileURLWithPath: info["AILDataDir"] as? String ?? "") }
    var isCurrent: Bool { (info["AILSourceVersion"] as? String) == version(of: source) }
    var look: Look {
        let i = info, d = Look.defaults(for: name)
        return Look(badge: i["AILBadge"] as? String ?? d.badge, color: i["AILColor"] as? String ?? d.color, imagePath: i["AILImage"] as? String,
                    glyph: i["AILGlyph"] as? Bool ?? false)
    }
}

func version(of app: URL) -> String? {
    (NSDictionary(contentsOf: app.appendingPathComponent("Contents/Info.plist")))?["CFBundleVersion"] as? String
}

@discardableResult
func run(_ tool: String, _ args: [String]) throws -> Data {
    let p = Process(), out = Pipe(), err = Pipe()
    p.executableURL = URL(fileURLWithPath: tool); p.arguments = args
    p.standardOutput = out; p.standardError = err
    try p.run()
    let data = out.fileHandleForReading.readDataToEndOfFile()
    let errData = err.fileHandleForReading.readDataToEndOfFile()
    p.waitUntilExit()
    guard p.terminationStatus == 0 else { throw failure("\((tool as NSString).lastPathComponent) failed: \(String(decoding: errData, as: UTF8.self))") }
    return data
}

func failure(_ message: String) -> NSError { NSError(domain: "AIGENT", code: 1, userInfo: [NSLocalizedDescriptionKey: message]) }
func slug(_ s: String) -> String { s.lowercased().map { $0.isLetter || $0.isNumber ? String($0) : "-" }.joined() }
func shQuote(_ s: String) -> String { "'" + s.replacingOccurrences(of: "'", with: "'\\''") + "'" }

extension NSColor {
    convenience init?(hex: String) {
        guard let v = UInt32(hex.trimmingCharacters(in: CharacterSet(charactersIn: "#")), radix: 16) else { return nil }
        self.init(srgbRed: CGFloat(v >> 16 & 0xff) / 255, green: CGFloat(v >> 8 & 0xff) / 255, blue: CGFloat(v & 0xff) / 255, alpha: 1)
    }
    var hex: String {
        let c = usingColorSpace(.sRGB) ?? self
        return String(format: "#%02X%02X%02X", Int(round(c.redComponent * 255)), Int(round(c.greenComponent * 255)), Int(round(c.blueComponent * 255)))
    }
}

// MARK: Icons

/// The profile icon: app icon (or custom image) with an optional colored badge, or a big emoji/letter tile.
/// Everything stays inside the standard icon shape (824 pt tile in a 1024 canvas); anything sticking out
/// makes macOS treat it as a legacy icon and shrink it into a grey frame.
func renderIcon(source: URL, look: Look) -> NSImage {
    let base = look.imagePath.flatMap { NSImage(contentsOfFile: $0) } ?? NSWorkspace.shared.icon(forFile: source.path)
    return NSImage(size: NSSize(width: 1024, height: 1024), flipped: false) { rect in
        if look.glyph {
            let tile = rect.insetBy(dx: 100, dy: 100)
            (NSColor(hex: look.color) ?? .systemBlue).setFill()
            NSBezierPath(roundedRect: tile, xRadius: 185, yRadius: 185).fill()
            let text = look.badge.isEmpty ? "?" : look.badge
            let size: CGFloat = text.count > 1 ? 340 : 500
            let str = NSAttributedString(string: text, attributes: [.font: NSFont.systemFont(ofSize: size, weight: .bold), .foregroundColor: NSColor.white])
            let s = str.size()
            str.draw(at: NSPoint(x: tile.midX - s.width / 2, y: tile.midY - s.height / 2))
            return true
        }
        if look.imagePath != nil {
            // Custom images: aspect-fill a standard macOS icon squircle.
            let tile = rect.insetBy(dx: 100, dy: 100)
            NSGraphicsContext.saveGraphicsState()
            NSBezierPath(roundedRect: tile, xRadius: 185, yRadius: 185).addClip()
            let s = max(tile.width / base.size.width, tile.height / base.size.height)
            let w = base.size.width * s, h = base.size.height * s
            base.draw(in: NSRect(x: tile.midX - w / 2, y: tile.midY - h / 2, width: w, height: h))
            NSGraphicsContext.restoreGraphicsState()
        } else {
            base.draw(in: rect)
        }
        guard !look.badge.isEmpty else { return true }
        let badge = NSRect(x: 565, y: 139, width: 320, height: 320)  // inside the tile's bottom-right corner
        (NSColor(hex: look.color) ?? .systemBlue).setFill(); NSBezierPath(ovalIn: badge).fill()
        NSColor.white.setStroke(); let ring = NSBezierPath(ovalIn: badge.insetBy(dx: 8, dy: 8)); ring.lineWidth = 16; ring.stroke()
        let size: CGFloat = look.badge.count > 1 ? 140 : 195
        let str = NSAttributedString(string: look.badge, attributes: [.font: NSFont.systemFont(ofSize: size, weight: .bold), .foregroundColor: NSColor.white])
        let s = str.size()
        str.draw(at: NSPoint(x: badge.midX - s.width / 2, y: badge.midY - s.height / 2))
        return true
    }
}

/// AIGENT's own icon: a violet squircle holding a 2×2 grid of app tiles, one lit up.
func renderAppIcon() -> NSImage {
    NSImage(size: NSSize(width: 1024, height: 1024), flipped: false) { rect in
        let tile = rect.insetBy(dx: 100, dy: 100)
        let shape = NSBezierPath(roundedRect: tile, xRadius: 185, yRadius: 185)
        NSGradient(starting: NSColor(hex: "#5B4BF5")!, ending: NSColor(hex: "#B347E8")!)!.draw(in: shape, angle: -60)
        let gap: CGFloat = 44, cell = (tile.width - 2 * 120 - gap) / 2
        for (i, (cx, cy)) in [(0, 1), (1, 1), (0, 0), (1, 0)].enumerated() {
            let r = NSRect(x: tile.minX + 120 + CGFloat(cx) * (cell + gap), y: tile.minY + 120 + CGFloat(cy) * (cell + gap), width: cell, height: cell)
            (i == 1 ? NSColor(hex: "#FF9F6B")! : NSColor.white.withAlphaComponent(0.92)).setFill()
            NSBezierPath(roundedRect: r, xRadius: 56, yRadius: 56).fill()
        }
        return true
    }
}

/// .icns = "icns" + total length, then one "ic10" (1024px PNG) entry + its length (big-endian).
func writeIcns(_ image: NSImage, to url: URL) throws {
    let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: 1024, pixelsHigh: 1024, bitsPerSample: 8, samplesPerPixel: 4,
                               hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
    image.draw(in: NSRect(x: 0, y: 0, width: 1024, height: 1024))
    NSGraphicsContext.restoreGraphicsState()
    let png = rep.representation(using: .png, properties: [:])!
    func be32(_ v: Int) -> Data { withUnsafeBytes(of: UInt32(v).bigEndian) { Data($0) } }
    try (Data("icns".utf8) + be32(16 + png.count) + Data("ic10".utf8) + be32(8 + png.count) + png).write(to: url)
}

/// Copy a user-picked image into the launcher's own folder so the profile doesn't depend on the original file.
func importImage(_ url: URL) throws -> String {
    guard let img = NSImage(contentsOf: url), let tiff = img.tiffRepresentation,
          let png = NSBitmapImageRep(data: tiff)?.representation(using: .png, properties: [:]) else { throw failure("That file isn't an image.") }
    try fm.createDirectory(at: iconsDir, withIntermediateDirectories: true)
    let dest = iconsDir.appendingPathComponent("\(UUID().uuidString).png")
    try png.write(to: dest)
    return dest.path
}

// MARK: Build

func isRunning(_ url: URL) -> NSRunningApplication? {
    NSWorkspace.shared.runningApplications.first { $0.bundleURL?.path == url.path }
}

/// Normal quit (⌘Q), then wait up to 30 s for the process to exit. Call off the main thread.
func quitAndWait(_ url: URL) throws {
    guard let app = isRunning(url) else { return }
    let pid = app.processIdentifier
    app.terminate()
    for _ in 0..<300 where kill(pid, 0) == 0 { usleep(100_000) }
    if kill(pid, 0) == 0 { throw failure("“\(app.localizedName ?? url.lastPathComponent)” didn't quit (it may be asking to save something).") }
}

/// Clone `source` into a profile bundle named `name`. Safe to re-run (used for updates and edits).
@discardableResult
func buildProfile(source: URL, name: String, dataDir existingDataDir: URL? = nil, look: Look? = nil) throws -> URL {
    let srcName = source.deletingPathExtension().lastPathComponent
    let display = "\(srcName) – \(name)"
    let dest = profilesDir.appendingPathComponent("\(display).app")
    let tmp = profilesDir.appendingPathComponent(".\(slug(display)).building.app")
    // Replacing a running app's bundle breaks it (Electron starts helpers from it later).
    if let app = isRunning(dest) { throw failure("Quit “\(app.localizedName ?? display)” first, then try again.") }
    // Same naming Multi-Claude used, so existing sign-ins carry over.
    let dataDir = existingDataDir ?? supportDir.appendingPathComponent("\(srcName)-\(name)")
    let look = look ?? .defaults(for: name)
    try fm.createDirectory(at: profilesDir, withIntermediateDirectories: true)
    try fm.createDirectory(at: dataDir, withIntermediateDirectories: true)
    try? fm.removeItem(at: tmp)
    try run("/bin/cp", ["-cR", source.path, tmp.path]) // APFS clone: ~0 extra disk

    let plistURL = tmp.appendingPathComponent("Contents/Info.plist")
    let plist = NSMutableDictionary(contentsOf: plistURL)!
    let realExec = plist["CFBundleExecutable"] as! String
    // Prefix kept from the old name: changing it would reset each profile's Keychain/privacy approvals.
    plist["CFBundleIdentifier"] = "local.ailauncher.\(slug(srcName)).\(slug(name))"
    plist["CFBundleDisplayName"] = display   // CFBundleName stays: Electron finds its helpers by it
    plist["CFBundleExecutable"] = "ail-launch"
    plist["CFBundleIconFile"] = "profile"
    // Leave links, file types and updates to the real app so clones don't steal them.
    for k in ["CFBundleIconName", "CFBundleURLTypes", "CFBundleDocumentTypes", "UTExportedTypeDeclarations", "SUFeedURL"] {
        plist.removeObject(forKey: k)
    }
    plist["SUEnableAutomaticChecks"] = false
    plist["AILName"] = name
    plist["AILSource"] = source.path
    plist["AILSourceVersion"] = version(of: source) ?? ""
    plist["AILDataDir"] = dataDir.path
    plist["AILBadge"] = look.badge
    plist["AILColor"] = look.color
    if let img = look.imagePath { plist["AILImage"] = img }
    plist["AILGlyph"] = look.glyph
    plist.write(to: plistURL, atomically: true)

    try writeIcns(renderIcon(source: source, look: look), to: tmp.appendingPathComponent("Contents/Resources/profile.icns"))

    // Wrapper: rebuild if the real app updated, otherwise start the real app with this profile's data folder.
    // claude-config: optional isolated Claude Code config dir (e.g. one migrated from another tool).
    let macOS = tmp.appendingPathComponent("Contents/MacOS")
    let script = """
    #!/bin/bash
    here="$(cd "$(dirname "$0")/../.." && pwd)"
    v=$(/usr/libexec/PlistBuddy -c "Print CFBundleVersion" \(shQuote(source.path))/Contents/Info.plist 2>/dev/null)
    if [ -n "$v" ] && [ "$v" != \(shQuote(version(of: source) ?? "")) ] && [ -x \(shQuote(launcherBinary)) ]; then
      \(shQuote(launcherBinary)) --rebuild "$here" --open >/dev/null 2>&1 &
      exit 0
    fi
    data=\(shQuote(dataDir.path))
    [ -d "$data/claude-config" ] && export CLAUDE_CONFIG_DIR="$data/claude-config"
    # Start the app as its own process (not exec) so macOS tracks its pid; exec breaks NSRunningApplication.
    "$(dirname "$0")/\(realExec)" --user-data-dir="$data" "$@" </dev/null >/dev/null 2>&1 &
    disown
    """
    let wrapper = macOS.appendingPathComponent("ail-launch")
    try script.write(to: wrapper, atomically: true, encoding: .utf8)
    try fm.setAttributes([.posixPermissions: 0o755], ofItemAtPath: wrapper.path)

    // Team-bound entitlements (keychain groups, app groups, push...) only work under the vendor's
    // signature, so keep just the plain com.apple.security.* ones.
    let entData = try run("/usr/bin/codesign", ["-d", "--entitlements", ":-", source.path])
    var ents = (try? PropertyListSerialization.propertyList(from: entData, format: nil) as? [String: Any]) ?? [:]
    ents = ents.filter { $0.key.hasPrefix("com.apple.security.") && $0.key != "com.apple.security.application-groups" }
    let entFile = fm.temporaryDirectory.appendingPathComponent("ail-\(UUID().uuidString).plist")
    try PropertyListSerialization.data(fromPropertyList: ents, format: .xml, options: 0).write(to: entFile)
    defer { try? fm.removeItem(at: entFile) }
    // A stable local certificate keeps Keychain/privacy approvals across rebuilds; ad hoc changes every build.
    let signer = (try? run("/usr/bin/security", ["find-certificate", "-c", signingCert])) != nil ? signingCert : "-"
    try run("/usr/bin/codesign", ["--force", "--deep", "--sign", signer, "--preserve-metadata=entitlements", tmp.path])
    try run("/usr/bin/codesign", ["--force", "--sign", signer, "--entitlements", entFile.path, macOS.appendingPathComponent(realExec).path])
    try run("/usr/bin/codesign", ["--force", "--sign", signer, "--entitlements", entFile.path, tmp.path])

    if fm.fileExists(atPath: dest.path) { try fm.removeItem(at: dest) }
    try fm.moveItem(at: tmp, to: dest)
    _ = try? run("/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister", ["-f", dest.path])
    return dest
}

/// Rebuild with the same data folder (and, unless given, the same look). A new name means a new bundle:
/// the old one goes to the Trash and its favorite slot moves to the new one.
@discardableResult
func update(_ p: Profile, name: String? = nil, look: Look? = nil) throws -> URL {
    if isRunning(p.url) != nil { throw failure("Quit “\(p.name)” first, then try again.") }
    let url = try buildProfile(source: p.source, name: name ?? p.name, dataDir: p.dataDir, look: look ?? p.look)
    if url.path != p.url.path {
        try fm.trashItem(at: p.url, resultingItemURL: nil)
        Favorites.replace(p.url.path, with: url.path)
    }
    return url
}

func loadProfiles() -> [Profile] {
    let urls = (try? fm.contentsOfDirectory(at: profilesDir, includingPropertiesForKeys: nil)) ?? []
    return urls.filter { $0.pathExtension == "app" && !$0.lastPathComponent.hasPrefix(".") }
        .map(Profile.init).sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
}

/// Ordered favorite app paths, stored as newline-joined text so SwiftUI's @AppStorage can bind to it.
enum Favorites {
    static let key = "favorites"
    static func list(_ raw: String) -> [String] { raw.split(separator: "\n").map(String.init) }
    static func replace(_ old: String, with new: String) {
        let raw = UserDefaults.standard.string(forKey: key) ?? ""
        UserDefaults.standard.set(list(raw).map { $0 == old ? new : $0 }.joined(separator: "\n"), forKey: key)
    }
}

func open(_ url: URL) { NSWorkspace.shared.openApplication(at: url, configuration: .init()) }
