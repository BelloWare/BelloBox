import Foundation

/// Local tools. One definition drives discovery, examples and both editor sizes.
enum AdditionalUtilityKind: String, CaseIterable, Identifiable {
    case calculator, units, numberBase, color, contrast, gradient, markdown
    case jsonPointer, jsonFlatten, jsonCode, sqlInsert, xmlJSON
    case unicode, stringEscape, extract, listSet, semver, subnet, chmod, hmac
    case jsonSchema, jsonMerge, jsonRedact, jsonLines, csvExplore, envFile, plist, sqlFormat, httpHeaders, cookies, certificate, sshKey, uuidInspect, bitwise, statistics, dateMath, aspectRatio, bezier, boxShadow, textTable
    var id: String { rawValue }
    var command: LauncherCommand { LauncherCommand(rawValue: rawValue)! }
    var title: String {
        if let definition = extendedDefinition { return definition.title }
        switch self {
        case .calculator: return "Calculator"
        case .units: return "Unit Converter"
        case .numberBase: return "Number Base Converter"
        case .color: return "Color Converter"
        case .contrast: return "Contrast Checker"
        case .gradient: return "CSS Gradient Builder"
        case .markdown: return "Markdown Preview"
        case .jsonPointer: return "JSON Pointer"
        case .jsonFlatten: return "Flatten & Unflatten JSON"
        case .jsonCode: return "JSON to Code"
        case .sqlInsert: return "SQL INSERT Builder"
        case .xmlJSON: return "XML to JSON"
        case .unicode: return "Unicode Inspector"
        case .stringEscape: return "String Literal Escaper"
        case .extract: return "Extract Links & Emails"
        case .listSet: return "List Set Operations"
        case .semver: return "Semantic Versions"
        case .subnet: return "IPv4 Subnet Calculator"
        case .chmod: return "Chmod Permissions"
        case .hmac: return "HMAC Signer"
        default: preconditionFailure("Missing utility metadata")
        }
    }
    var subtitle: String {
        if let definition = extendedDefinition { return definition.subtitle }
        switch self {
        case .calculator: return "Evaluate arithmetic, powers, constants, and math functions"
        case .units: return "Convert length, mass, temperature, data, duration, and speed"
        case .numberBase: return "Convert exact integers between binary, octal, decimal, and hex"
        case .color: return "Convert HEX, RGB, and HSL with a live color swatch"
        case .contrast: return "Check two colors against WCAG AA and AAA contrast thresholds"
        case .gradient: return "Design a two-color linear gradient and copy its CSS"
        case .markdown: return "Preview headings, lists, quotes, and code; export safe HTML"
        case .jsonPointer: return "Read a JSON value by its RFC 6901 pointer"
        case .jsonFlatten: return "Round-trip nested JSON through typed path entries"
        case .jsonCode: return "Infer TypeScript interfaces or Swift Codable models from JSON"
        case .sqlInsert: return "Turn JSON records into quoted SQL statements, without executing"
        case .xmlJSON: return "Convert XML to ordered JSON with attributes and mixed text intact"
        case .unicode: return "Inspect code points, UTF encodings, names, and normalization"
        case .stringEscape: return "Quote JSON, Swift, or shell strings without running code"
        case .extract: return "Collect unique links or email addresses from a block of text"
        case .listSet: return "Find common, combined, or different items in two line lists"
        case .semver: return "Compare and sort strict semantic versions, including prereleases"
        case .subnet: return "Inspect a CIDR network, netmask, address range, and host count"
        case .chmod: return "Build Unix permissions with an interactive read/write/execute grid"
        case .hmac: return "Sign text locally with SHA-256 or SHA-512 and an ephemeral key"
        default: preconditionFailure("Missing utility metadata")
        }
    }
    var symbol: String {
        if let definition = extendedDefinition { return definition.symbol }
        switch self {
        case .calculator: return "plus.forwardslash.minus"
        case .units: return "ruler"
        case .numberBase: return "number.square"
        case .color: return "paintpalette"
        case .contrast: return "circle.lefthalf.filled"
        case .gradient: return "rectangle.leadinghalf.inset.filled"
        case .markdown: return "doc.richtext"
        case .jsonPointer: return "scope"
        case .jsonFlatten: return "square.stack.3d.down.right"
        case .jsonCode: return "chevron.left.forwardslash.chevron.right"
        case .sqlInsert: return "externaldrive.badge.plus"
        case .xmlJSON: return "doc.badge.gearshape"
        case .unicode: return "character.cursor.ibeam"
        case .stringEscape: return "text.quote"
        case .extract: return "line.3.horizontal.decrease.circle"
        case .listSet: return "circle.grid.2x1"
        case .semver: return "tag"
        case .subnet: return "point.3.connected.trianglepath.dotted"
        case .chmod: return "lock.shield"
        case .hmac: return "signature"
        default: preconditionFailure("Missing utility metadata")
        }
    }
    enum Group: String, CaseIterable { case math = "Math & numbers", design = "Color & design", data = "Data & code", text = "Text & debugging", security = "Network & security" }
    var group: Group {
        if let definition = extendedDefinition { return definition.group }
        switch self {
        case .calculator, .units, .numberBase: return .math
        case .color, .contrast, .gradient, .markdown: return .design
        case .jsonPointer, .jsonFlatten, .jsonCode, .sqlInsert, .xmlJSON: return .data
        case .unicode, .stringEscape, .extract, .listSet, .semver: return .text
        case .subnet, .chmod, .hmac: return .security
        default: preconditionFailure("Missing utility metadata")
        }
    }
    var keywords: String { subtitle + " " + group.rawValue + " " + rawValue + " " + (self == .hmac ? "hash authentication digest secret key" : self == .numberBase ? "radix hexadecimal binary octal decimal" : "") }
    var inputLabel: String {
        if let definition = extendedDefinition { return definition.inputLabel }
        switch self {
        case .calculator: return "Expression · radians · e.g. sqrt(144) + 2^3"
        case .units: return "Value"
        case .numberBase: return "Integer · optional sign and matching 0x / 0b / 0o prefix"
        case .color, .contrast, .gradient: return self == .contrast ? "Text color" : self == .gradient ? "Start color" : "Color · HEX, rgb(), or hsl()"
        case .jsonPointer, .jsonCode, .sqlInsert: return "JSON input"
        case .jsonFlatten: return "JSON or typed path entries"
        case .markdown: return "Markdown · images stay offline"
        case .xmlJSON: return "XML · DTDs and external entities are not accepted"
        case .listSet: return "First list · one item per line"
        case .semver: return "Versions · one per line"
        case .subnet: return "IPv4 / prefix · e.g. 192.168.1.42/24"
        case .chmod: return "Octal or rwx permissions · e.g. 755 or rwxr-xr-x"
        case .hmac: return "Message"
        default: return "Input"
        }
    }
    var multiline: Bool { extendedDefinition?.multiline ?? ![.calculator, .units, .numberBase, .color, .contrast, .gradient, .subnet, .chmod].contains(self) }
    var example: String {
        if let definition = extendedDefinition { return definition.example }
        switch self {
        case .calculator: return "(125 + 75) * 1.08"
        case .units: return "1024"
        case .numberBase: return "9007199254740993"
        case .color, .contrast, .gradient: return "#B84D16"
        case .markdown: return "# Ship something useful\n\nA **small tool** can save a lot of time.\n\n- Works offline\n- Keeps your draft\n\n```swift\nlet greeting = \"Hello, Bello Box\"\n```"
        case .jsonPointer: return "{\"users\":[{\"name\":\"Ada\",\"id\":9007199254740993}]}"
        case .jsonFlatten: return "{\"user\":{\"name\":\"Ada\",\"roles\":[\"developer\"]},\"empty\":{},\"active\":true}"
        case .jsonCode: return "[{\"id\":1,\"name\":\"Ada\",\"active\":true},{\"id\":2,\"name\":\"Lin\",\"tags\":[\"swift\"]}]"
        case .sqlInsert: return "[{\"id\":1,\"name\":\"Ada\",\"active\":true},{\"id\":2,\"name\":\"O'Reilly\",\"active\":false}]"
        case .xmlJSON: return "<note priority=\"high\">Hello <b>Ada</b>!<tag>swift</tag><tag>macOS</tag></note>"
        case .unicode: return "Hello 👩🏽‍💻 café e\u{301}"
        case .stringEscape: return "Hello \"Bello Box\"\nA second line\twith a tab."
        case .extract: return "Read https://example.com/docs?lang=swift or mail hello@example.com.\nAlso https://example.com/docs?lang=swift"
        case .listSet: return "Swift\nRust\nPython"
        case .semver: return "1.0.0\n1.0.0-rc.2\n1.0.0-rc.10\n2.0.0\n1.0.0+build.7"
        case .subnet: return "192.168.1.42/24"
        case .chmod: return "755"
        case .hmac: return "what do ya want for nothing?"
        default: preconditionFailure("Missing utility metadata")
        }
    }
    struct Option: Identifiable {
        let id: String, label: String, choices: [String]
        var initial: String { choices[0] }
    }
    struct Field: Identifiable {
        let id: String, label: String, initial: String
    }
    var options: [Option] {
        if let definition = extendedDefinition { return definition.options }
        switch self {
        case .units: return [.init(id: "from", label: "From", choices: UnitTool.units.map(\.name)), .init(id: "to", label: "To", choices: UnitTool.units.map(\.name))]
        case .numberBase: return [.init(id: "base", label: "Input base", choices: ["10", "2", "8", "16"]), .init(id: "outputBase", label: "Output", choices: ["All bases", "10", "16", "2", "8"])]
        case .color: return [.init(id: "format", label: "Copy format", choices: ["All formats", "HEX", "RGB", "HSL"])]
        case .jsonFlatten: return [.init(id: "mode", label: "Operation", choices: ["Flatten", "Unflatten"])]
        case .jsonCode: return [.init(id: "language", label: "Language", choices: ["TypeScript", "Swift"])]
        case .sqlInsert: return [.init(id: "dialect", label: "Dialect", choices: ["PostgreSQL", "SQLite"])]
        case .unicode: return [.init(id: "mode", label: "Output", choices: ["Inspect", "NFC", "NFD", "NFKC", "NFKD"])]
        case .stringEscape: return [.init(id: "mode", label: "Format", choices: ["JSON quote", "JSON unquote", "Swift literal", "Shell quote"])]
        case .extract: return [.init(id: "mode", label: "Find", choices: ["Links", "Emails"]), .init(id: "duplicates", label: "Duplicates", choices: ["Unique", "Keep all"])]
        case .listSet: return [.init(id: "mode", label: "Operation", choices: ["Union", "Intersection", "A − B", "B − A", "Symmetric difference"]), .init(id: "matching", label: "Match", choices: ["Exact", "Trim", "Trim & ignore case"])]
        case .semver: return [.init(id: "mode", label: "Order", choices: ["Ascending", "Descending", "Compare first two"])]
        case .hmac: return [.init(id: "algorithm", label: "Algorithm", choices: ["SHA-256", "SHA-512"]), .init(id: "keyFormat", label: "Key", choices: ["Text", "Hex"]), .init(id: "encoding", label: "Output", choices: ["Hex", "Base64"])]
        default: return []
        }
    }
    var fields: [Field] {
        if let definition = extendedDefinition { return definition.fields }
        switch self {
        case .contrast: return [.init(id: "background", label: "Background", initial: "#FFFFFF")]
        case .gradient: return [.init(id: "end", label: "End color", initial: "#F6B76B"), .init(id: "angle", label: "Angle (°)", initial: "135")]
        case .jsonPointer: return [.init(id: "pointer", label: "Pointer · empty = root", initial: "")]
        case .jsonCode: return [.init(id: "name", label: "Root type", initial: "Root")]
        case .sqlInsert: return [.init(id: "table", label: "Table name", initial: "records")]
        default: return []
        }
    }
    var defaults: [String: String] {
        var values = Dictionary(uniqueKeysWithValues: options.map { ($0.id, $0.initial) } + fields.map { ($0.id, $0.initial) })
        if self == .units { values["from"] = "KiB"; values["to"] = "MiB" }
        return values
    }
}

enum AdditionalUtilityEngine {
    static func run(_ kind: AdditionalUtilityKind, input: String, second: String = "", options: [String: String] = [:]) throws -> WorkbenchResult {
        try UtilityLimits.check(input); try UtilityLimits.check(second)
        for value in options.values { try UtilityLimits.check(value) }
        let options = kind.defaults.merging(options) { _, new in new }
        let result: WorkbenchResult
        switch kind {
        case .calculator: result = try MathTool.calculate(input)
        case .units: result = try UnitTool.convert(input, from: options["from"]!, to: options["to"]!)
        case .numberBase: result = try NumberBaseTool.convert(input, base: Int(options["base"]!) ?? 10, outputBase: Int(options["outputBase"]!))
        case .color, .contrast, .gradient: result = try ColorTool.run(kind, input: input, options: options)
        case .markdown: result = try MarkdownTool.render(input)
        case .jsonPointer, .jsonFlatten, .jsonCode, .sqlInsert, .xmlJSON: result = try StructuredUtility.run(kind, input: input, options: options)
        case .unicode, .stringEscape, .extract, .listSet, .semver: result = try TextUtility.run(kind, input: input, second: second, options: options)
        case .subnet, .chmod, .hmac: result = try SecurityUtility.run(kind, input: input, second: second, options: options)
        default: result = try ExtendedUtilityEngine.run(kind, input: input, second: second, options: options)
        }
        guard result.text.utf8.count <= 4_096_000 else { throw UtilityError("The result exceeds 4 MB. Use a smaller input.") }
        try Task.checkCancellation()
        return result
    }
}
