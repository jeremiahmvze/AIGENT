import SwiftUI
import AppKit
import Combine
import ServiceManagement
import UniformTypeIdentifiers

let installedApps = knownApps.map { URL(fileURLWithPath: "/Applications/\($0).app") }.filter { fm.fileExists(atPath: $0.path) }

struct ContentView: View {
    @State private var profiles = loadProfiles()
    @State private var running: Set<String> = []
    @State private var newName = ""
    @State private var newSource = knownApps.first!
    @State private var busy: String?
    @State private var error: String?
    @State private var editing: Profile?
    @AppStorage(Favorites.key) private var favoritesRaw = ""
    @AppStorage("expandFavorites") private var expandFavorites = true
    @AppStorage("expandApps") private var expandApps = true
    @AppStorage("expandProfiles") private var expandProfiles = true
    @AppStorage("allIncludesDefaults") private var allIncludesDefaults = false
    private let timer = Timer.publish(every: 2, on: .main, in: .common).autoconnect()

    private var allItems: [URL] { installedApps + profiles.map(\.url) }
    // Favorites that still exist, in the user's order.
    private var favorites: [URL] {
        let byPath = Dictionary(allItems.map { ($0.path, $0) }, uniquingKeysWith: { a, _ in a })
        return Favorites.list(favoritesRaw).compactMap { byPath[$0] }
    }
    private var bulkTargets: [URL] { (allIncludesDefaults ? installedApps : []) + profiles.map(\.url) }

    var body: some View {
        VStack(spacing: 0) {
            List {
                if !favorites.isEmpty {
                    Section("Favorites", isExpanded: $expandFavorites) {
                        ForEach(favorites, id: \.path) { row($0) }
                            .onMove { from, to in
                                var list = favorites.map(\.path); list.move(fromOffsets: from, toOffset: to)
                                favoritesRaw = list.joined(separator: "\n")
                            }
                    }
                }
                Section("Apps", isExpanded: $expandApps) {
                    ForEach(installedApps.filter { !isFavorite($0) }, id: \.path) { row($0) }
                }
                Section("Profiles", isExpanded: $expandProfiles) {
                    if profiles.isEmpty { Text("No profiles yet. Add one below.").foregroundStyle(.secondary) }
                    ForEach(profiles.filter { !isFavorite($0.url) }) { row($0.url) }
                }
            }
            .listStyle(.sidebar)
            Divider()
            HStack {
                Picker("", selection: $newSource) {
                    ForEach(installedApps, id: \.self) { Text($0.deletingPathExtension().lastPathComponent).tag($0.deletingPathExtension().lastPathComponent) }
                }.labelsHidden().fixedSize()
                TextField("New profile name (e.g. Work)", text: $newName).onSubmit(add)
                Button(busy ?? "Add Profile", action: add).disabled(busy != nil || newName.trimmingCharacters(in: .whitespaces).isEmpty)
            }.padding(10)
        }
        .frame(minWidth: 480, minHeight: 420)
        .toolbar {
            ToolbarItemGroup {
                Button { launchAll() } label: { Label("Launch All", systemImage: "play.fill") }
                    .help("Open every profile" + (allIncludesDefaults ? " and default app" : ""))
                Button { closeAll() } label: { Label("Close All", systemImage: "stop.fill") }
                    .help("Quit every running profile" + (allIncludesDefaults ? " and default app" : ""))
            }
        }
        .onReceive(timer) { _ in refresh() }
        .onAppear(perform: refresh)
        .sheet(item: $editing) { p in
            ProfileEditor(profile: p) { name, look in work("Saving…") { try update(p, name: name, look: look) } }
        }
        .alert("Something went wrong", isPresented: .constant(error != nil)) { Button("OK") { error = nil } } message: { Text(error ?? "") }
    }

    // MARK: Rows

    private func row(_ url: URL) -> some View {
        let profile = url.path.hasPrefix(profilesDir.path) ? Profile(url: url) : nil
        let isOn = running.contains(url.path)
        return HStack {
            Image(nsImage: NSImage(contentsOf: url.appendingPathComponent("Contents/Resources/profile.icns")) ?? NSWorkspace.shared.icon(forFile: url.path))
                .resizable().frame(width: 32, height: 32)
            VStack(alignment: .leading) {
                Text(profile?.name ?? url.deletingPathExtension().lastPathComponent)
                Text(profile.map { $0.source.deletingPathExtension().lastPathComponent + ($0.isCurrent ? "" : " · update pending") } ?? "Default profile")
                    .font(.caption).foregroundStyle(.secondary)
            }
            Spacer()
            if isOn { Circle().fill(.green).frame(width: 8, height: 8).help("Running").accessibilityLabel("Running") }
            if let p = profile, !p.isCurrent {
                Button("Update") { work("Updating…") { try quitAndWait(url); open(try update(p)) } }
                    .help("Rebuild from the newly installed \(p.source.deletingPathExtension().lastPathComponent)" + (isOn ? " (quits and reopens it)" : ""))
            }
            if isOn {
                Button("Restart") { work("Restarting…") { try quitAndWait(url); open(url) } }
                    .help("Quit and reopen")
            }
            Button(isOn ? "Show" : "Launch") { open(url) }
        }
        .padding(.vertical, 2)
        .contextMenu {
            Button(isFavorite(url) ? "Remove from Favorites" : "Add to Favorites") { toggleFavorite(url) }
            if let p = profile {
                Divider()
                Button("Edit Name & Icon…") { editing = p }
                Button("Rebuild") { work("Rebuilding…") { try update(p) } }
                Button("Show Data Folder") { NSWorkspace.shared.activateFileViewerSelecting([p.dataDir]) }
                Divider()
                Button("Delete Profile…") { delete(p) }
            }
        }
    }

    private func isFavorite(_ url: URL) -> Bool { Favorites.list(favoritesRaw).contains(url.path) }

    private func toggleFavorite(_ url: URL) {
        var list = Favorites.list(favoritesRaw).filter { $0 != url.path }
        if !isFavorite(url) { list.append(url.path) }
        favoritesRaw = list.joined(separator: "\n")
    }

    // MARK: Actions

    private func refresh() {
        profiles = loadProfiles()
        running = Set(NSWorkspace.shared.runningApplications.compactMap { $0.bundleURL?.path })
    }

    private func work(_ label: String, _ job: @escaping () throws -> Void) {
        busy = label
        DispatchQueue.global().async {
            let e: Error? = { do { try job(); return nil } catch { return error } }()
            DispatchQueue.main.async { busy = nil; error = e?.localizedDescription; refresh() }
        }
    }

    private func add() {
        let name = newName.trimmingCharacters(in: .whitespaces)
        guard !name.isEmpty, busy == nil, !name.contains("/") else { return }
        let src = URL(fileURLWithPath: "/Applications/\(newSource).app")
        work("Building…") { open(try buildProfile(source: src, name: name)) }
        newName = ""
    }

    private func launchAll() {
        for url in bulkTargets where !running.contains(url.path) { open(url) }
    }

    private func closeAll() {
        let targets = Set(bulkTargets.map(\.path))
        let apps = NSWorkspace.shared.runningApplications.filter { targets.contains($0.bundleURL?.path ?? "") }
        guard !apps.isEmpty else { return }
        let a = NSAlert()
        a.messageText = "Quit \(apps.count) app\(apps.count == 1 ? "" : "s")?"
        a.informativeText = apps.compactMap(\.localizedName).sorted().joined(separator: "\n")
        a.addButton(withTitle: "Quit All"); a.addButton(withTitle: "Cancel")
        guard a.runModal() == .alertFirstButtonReturn else { return }
        apps.forEach { $0.terminate() } // normal quit (⌘Q), so apps save their state
    }

    private func delete(_ p: Profile) {
        let a = NSAlert()
        a.messageText = "Delete “\(p.name)”?"
        a.informativeText = "The profile app moves to the Trash. Its data folder (sign-in, sessions) is kept, so re-adding the same name restores it."
        a.addButton(withTitle: "Move to Trash"); a.addButton(withTitle: "Cancel")
        guard a.runModal() == .alertFirstButtonReturn else { return }
        favoritesRaw = Favorites.list(favoritesRaw).filter { $0 != p.url.path }.joined(separator: "\n")
        NSWorkspace.shared.recycle([p.url]) { _, _ in DispatchQueue.main.async { refresh() } }
    }
}

// MARK: Edit name & icon

let quickEmoji = ["💼", "🏠", "💻", "🧪", "🛠️", "📚", "🎓", "🧠", "💡", "🚀",
                  "⭐️", "❤️", "🔥", "⚡️", "🌙", "☀️", "🌈", "🌊", "🌲", "🍀",
                  "🐶", "🐱", "🦊", "🐻", "🐼", "🦉", "🐙", "🦄", "🤖", "👾",
                  "🎨", "🎵", "🎮", "📷", "✈️", "💰", "📈", "🏋️", "🛒", "🔒"]

struct ProfileEditor: View {
    let profile: Profile
    let save: (String, Look) -> Void
    @Environment(\.dismiss) private var dismiss
    @State private var name: String
    @State private var badge: String
    @State private var color: Color
    @State private var imagePath: String?
    @State private var glyph: Bool
    @State private var error: String?
    @FocusState private var badgeFocused: Bool

    init(profile: Profile, save: @escaping (String, Look) -> Void) {
        self.profile = profile; self.save = save
        let look = profile.look
        _name = State(initialValue: profile.name)
        _badge = State(initialValue: look.badge)
        _color = State(initialValue: Color(nsColor: NSColor(hex: look.color) ?? .systemBlue))
        _imagePath = State(initialValue: look.imagePath)
        _glyph = State(initialValue: look.glyph)
    }

    private var maxBadge: Int { glyph ? 1 : 2 }
    private var look: Look { Look(badge: String(badge.prefix(maxBadge)), color: NSColor(color).hex, imagePath: imagePath, glyph: glyph) }
    private var cleanName: String { name.trimmingCharacters(in: .whitespaces) }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Edit Profile").font(.headline)
            HStack(alignment: .top, spacing: 20) {
                Image(nsImage: renderIcon(source: profile.source, look: look))
                    .resizable().frame(width: 128, height: 128)
                    .accessibilityLabel("Icon preview")
                Form {
                    TextField("Name", text: $name)
                    HStack {
                        TextField("Emoji or letter", text: $badge, prompt: Text(glyph ? "1 character" : "1–2 characters, or empty"))
                            .focused($badgeFocused)
                            .onChange(of: badge) { _, v in if v.count > maxBadge { badge = String(v.suffix(maxBadge)) } }
                            .onChange(of: glyph) { _, _ in if badge.count > maxBadge { badge = String(badge.prefix(maxBadge)) } }
                        Button("More…") { badgeFocused = true; DispatchQueue.main.async { NSApp.orderFrontCharacterPalette(nil) } }
                            .help("All emoji and symbols (or press fn/🌐 E)")
                    }
                    Picker("Show as", selection: $glyph) {
                        Text("Badge").tag(false)
                        Text("Whole Icon").tag(true)
                    }.pickerStyle(.segmented)
                    ColorPicker(glyph ? "Background" : "Badge color", selection: $color, supportsOpacity: false)
                    HStack(spacing: 6) {
                        ForEach(palette, id: \.self) { hex in
                            Button { color = Color(nsColor: NSColor(hex: hex)!) } label: {
                                Circle().fill(Color(nsColor: NSColor(hex: hex)!)).frame(width: 18, height: 18)
                            }.buttonStyle(.plain).accessibilityLabel("Color \(hex)")
                        }
                    }
                    if !glyph {
                        HStack {
                            Button("Choose Image…", action: chooseImage)
                            if imagePath != nil { Button("Use App Icon") { imagePath = nil } }
                        }
                    }
                }
            }
            // Reminders-style quick picker.
            LazyVGrid(columns: Array(repeating: GridItem(.fixed(40), spacing: 8), count: 10), spacing: 8) {
                ForEach(quickEmoji, id: \.self) { e in
                    Button { badge = e } label: {
                        Text(e).font(.system(size: 22)).frame(width: 40, height: 40)
                            .background(Circle().fill(badge == e ? Color.accentColor.opacity(0.3) : Color.secondary.opacity(0.12)))
                    }.buttonStyle(.plain).accessibilityLabel("Use \(e)")
                }
            }
            if let error { Text(error).foregroundStyle(.red).font(.caption) }
            Text("Saving rebuilds the profile. Sign-in and sessions stay the same; quit it first if it's running.")
                .font(.caption).foregroundStyle(.secondary)
            HStack {
                Spacer()
                Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
                Button("Save") { save(cleanName, look); dismiss() }
                    .keyboardShortcut(.defaultAction)
                    .disabled(cleanName.isEmpty || cleanName.contains("/"))
            }
        }
        .padding(20)
        .frame(width: 520)
    }

    private func chooseImage() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.image]
        guard panel.runModal() == .OK, let url = panel.url else { return }
        do { imagePath = try importImage(url); error = nil } catch { self.error = error.localizedDescription }
    }
}

// MARK: Settings

struct SettingsView: View {
    @AppStorage("allIncludesDefaults") private var allIncludesDefaults = false
    @State private var openAtLogin = SMAppService.mainApp.status == .enabled
    @State private var error: String?

    var body: some View {
        Form {
            Toggle("Open at Login", isOn: $openAtLogin)
                .onChange(of: openAtLogin) { _, on in
                    do { try on ? SMAppService.mainApp.register() : SMAppService.mainApp.unregister(); error = nil }
                    catch { self.error = error.localizedDescription; openAtLogin = SMAppService.mainApp.status == .enabled }
                }
            Toggle("Include default Claude and ChatGPT in Launch All / Close All", isOn: $allIncludesDefaults)
            if let error { Text(error).foregroundStyle(.red).font(.caption) }
        }
        .padding(20)
        .frame(width: 460)
    }
}

struct AIGENTApp: App {
    var body: some Scene {
        Window("AIGENT", id: "main") { ContentView() }
            .defaultSize(width: 560, height: 560)
        Settings { SettingsView() }
    }
}
