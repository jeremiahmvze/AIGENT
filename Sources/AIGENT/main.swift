// AIGENT: run several signed-in copies ("profiles") of Electron AI apps side by side.
import AppKit

// Command-line mode (used by profile wrappers after the real app updates, and by build.sh).
let args = CommandLine.arguments
func cli(_ job: () throws -> Void) -> Never {
    do { try job(); exit(0) } catch { FileHandle.standardError.write(Data("\(error.localizedDescription)\n".utf8)); exit(1) }
}
func arg(_ flag: String, _ n: Int = 1) -> [String]? {
    guard let i = args.firstIndex(of: flag), i + n < args.count else { return nil }
    return Array(args[(i + 1)...(i + n)])
}

if let a = arg("--rebuild") {
    cli {
        let url = try update(Profile(url: URL(fileURLWithPath: a[0])))
        if args.contains("--open") { open(url); sleep(2) }
    }
}
if let a = arg("--add", 2) {
    cli { print(try buildProfile(source: URL(fileURLWithPath: "/Applications/\(a[0]).app"), name: a[1]).path) }
}
if let a = arg("--rename", 2) {
    cli { print(try update(Profile(url: URL(fileURLWithPath: a[0])), name: a[1]).path) }
}
if args.contains("--self-test") {
    cli {
        assert(NSColor(hex: "#BF5AF2")!.hex == "#BF5AF2", "hex round-trip")
        assert(Look.defaults(for: "Personal") == Look.defaults(for: "Personal"), "stable defaults")
        assert(Look.defaults(for: "2 Dev").badge == "2" && Look.defaults(for: "").badge == "?", "badge letter")
        assert(Favorites.list("a\nb\n") == ["a", "b"], "favorites parse")
        assert(slug("Archive - Development") == "archive---development", "bundle id slug")
        assert(Look(badge: "🦊", color: "#FF375F", glyph: true).badge.count == 1, "emoji is one character")
        let l = Look(badge: "🦊", color: "#FF375F", imagePath: nil, glyph: true, framed: true)
        assert(Look.from(l.plist, name: "x") == l, "look plist round-trip")
        assert(Look.from(Look(badge: "A", color: "#000000", imagePath: "/a.png").plist.merging(l.plist) { $1 }, name: "x").imagePath == nil, "queued change clears image")
        print("self-test passed")
    }
}
if let a = arg("--write-app-icon") {
    cli { try writeIcns(renderAppIcon(), to: URL(fileURLWithPath: a[0])) }
}

AIGENTApp.main()
