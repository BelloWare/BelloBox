import Foundation

/// The next twenty utilities use the same session, controls and discovery model.
struct ExtendedUtilityDefinition {
    let title: String
    let subtitle: String
    let symbol: String
    let group: AdditionalUtilityKind.Group
    let inputLabel: String
    let example: String
    var multiline = true
    var secondLabel: String? = nil
    var secondExample = ""
    var options: [AdditionalUtilityKind.Option] = []
    var fields: [AdditionalUtilityKind.Field] = []
    var height: CGFloat = 304
}

extension AdditionalUtilityKind {
    var extendedDefinition: ExtendedUtilityDefinition? {
        switch self {
        case .jsonSchema:
            return .init(title: "JSON Schema Validator", subtitle: "Check types, required fields, bounds, and local schema references", symbol: "checkmark.seal", group: .data,
                inputLabel: "JSON document", example: #"{"name":"Ada","age":37,"tags":["swift"]}"#,
                secondLabel: "Schema · supported keywords only", secondExample: #"{"type":"object","required":["name","age"],"properties":{"name":{"type":"string","minLength":1},"age":{"type":"integer","minimum":0},"tags":{"type":"array","items":{"type":"string"}}},"additionalProperties":false}"#)
        case .jsonMerge:
            return .init(title: "JSON Merge Patch", subtitle: "Apply an RFC 7396 patch locally; null removes object fields", symbol: "arrow.triangle.merge", group: .data,
                inputLabel: "Original JSON", example: #"{"name":"Ada","profile":{"city":"London","old":true},"tags":["v1"]}"#,
                secondLabel: "Merge patch", secondExample: #"{"profile":{"city":"Singapore","old":null},"tags":["v2"]}"#)
        case .jsonRedact:
            return .init(title: "JSON Field Redactor", subtitle: "Replace matching field names at every depth before sharing JSON", symbol: "eye.slash", group: .data,
                inputLabel: "JSON · review the result before sharing", example: #"{"user":{"name":"Ada","email":"ada@example.com"},"token":"example-token","active":true}"#,
                fields: [.init(id: "keys", label: "Field names (comma separated)", initial: "password,token,secret,api_key,email")])
        case .jsonLines:
            return .init(title: "JSON Lines", subtitle: "Validate NDJSON records and convert to or from a JSON array", symbol: "list.bullet.rectangle", group: .data,
                inputLabel: "One complete JSON value per line", example: "{\"id\":1,\"event\":\"start\"}\n{\"id\":2,\"event\":\"done\"}",
                options: [.init(id: "mode", label: "Convert", choices: ["Lines → Array", "Array → Lines"])])
        case .csvExplore:
            return .init(title: "CSV Explorer", subtitle: "Filter rows, choose columns, remove duplicates, and copy CSV", symbol: "tablecells.badge.ellipsis", group: .data,
                inputLabel: "CSV · first row contains column names", example: "name,team,city\nAda,Platform,London\nLin,Design,Singapore\nSam,Platform,Tokyo\nAda,Platform,London",
                options: [.init(id: "delimiter", label: "Separator", choices: ["Comma", "Tab", "Semicolon"]), .init(id: "rows", label: "Rows", choices: ["All", "Unique"])],
                fields: [.init(id: "filter", label: "Contains", initial: ""), .init(id: "columns", label: "Columns · comma separated", initial: "")], height: 328)
        case .envFile:
            return .init(title: "Environment File", subtitle: "Validate .env assignments and convert JSON without evaluating variables", symbol: "terminal", group: .data,
                inputLabel: "One KEY=value per line · values stay literal", example: "# Development\nAPP_NAME=\"Bello Box\"\nPORT=8080\nAPI_URL=https://example.com\nGREETING='Hello ${USER}'",
                options: [.init(id: "mode", label: "Convert", choices: ["Env → JSON", "JSON → Env"])])
        case .plist:
            return .init(title: "Property List Converter", subtitle: "Convert XML plist and typed JSON, preserving dates and binary data", symbol: "list.bullet.indent", group: .data,
                inputLabel: "XML plist or typed JSON", example: "<plist version=\"1.0\"><dict><key>Name</key><string>Bello Box</string><key>Enabled</key><true/><key>Created</key><date>2026-09-12T00:00:00Z</date></dict></plist>",
                options: [.init(id: "mode", label: "Convert", choices: ["Plist → JSON", "JSON → Plist"])])
        case .sqlFormat:
            return .init(title: "SQL Formatter", subtitle: "Lay out SQL clauses while keeping literals, comments, and quoted names", symbol: "text.alignleft", group: .data,
                inputLabel: "SQL · formats text; does not execute or validate queries", example: "select u.name, count(*) as total from users u left join orders o on o.user_id=u.id where u.active=true group by u.name order by total desc;",
                options: [.init(id: "keywords", label: "Keywords", choices: ["Uppercase", "Preserve"]), .init(id: "dialect", label: "Dialect", choices: ["PostgreSQL", "SQLite", "MySQL"])])
        case .httpHeaders:
            return .init(title: "HTTP Header Inspector", subtitle: "Inspect raw request or response headers with duplicate fields intact", symbol: "network.badge.shield.half.filled", group: .security,
                inputLabel: "Header block · optional request/status line", example: "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nCache-Control: public, max-age=3600\r\nVary: Accept-Encoding\r\nSet-Cookie: theme=dark; Path=/; Secure\r\nSet-Cookie: locale=en; Path=/\r\n")
        case .cookies:
            return .init(title: "Cookie Inspector", subtitle: "Inspect Cookie or Set-Cookie fields without accessing browser storage", symbol: "circle.hexagongrid", group: .security,
                inputLabel: "Cookie header or one Set-Cookie per line", example: "session=example; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=3600",
                options: [.init(id: "mode", label: "Header", choices: ["Set-Cookie", "Cookie"])])
        case .certificate:
            return .init(title: "Certificate Inspector", subtitle: "Read PEM certificate subjects, dates, extensions, and SHA-256 fingerprints", symbol: "checkmark.shield", group: .security,
                inputLabel: "PEM certificates · inspection only, no trust evaluation", example: CertificateTool.example)
        case .sshKey:
            return .init(title: "SSH Public Key Inspector", subtitle: "Inspect OpenSSH RSA, Ed25519, and ECDSA keys and their fingerprints", symbol: "key.viewfinder", group: .security,
                inputLabel: "OpenSSH public key · private keys are not accepted", example: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIIjHzQuE4mVVNZ33vbXM4iAWWOlxbTI01lJAojWyBLpg example@belloware")
        case .uuidInspect:
            return .init(title: "UUID Inspector", subtitle: "Read UUID versions, variants, bytes, and v1/v6/v7 timestamps", symbol: "number.square.fill", group: .security,
                inputLabel: "UUID · hyphenated, compact, or urn:uuid:", example: "017f22e2-79b0-7cc3-98c4-dc0c0c07398f", multiline: false, height: 256)
        case .bitwise:
            return .init(title: "Bitwise Calculator", subtitle: "Inspect fixed-width AND, OR, XOR, shifts, rotations, and signed results", symbol: "switch.2", group: .math,
                inputLabel: "Operand A · decimal, 0x hex, or 0b binary", example: "0b10101010", multiline: false,
                options: [.init(id: "operation", label: "Operation", choices: ["AND", "OR", "XOR", "NOT", "Left shift", "Right shift", "Rotate left", "Rotate right"]), .init(id: "width", label: "Bits", choices: ["8", "16", "32", "64"])],
                fields: [.init(id: "operand", label: "Operand B / shift", initial: "0b11001100")], height: 328)
        case .statistics:
            return .init(title: "Statistics Calculator", subtitle: "Summarize a number series with percentiles, spread, and a histogram", symbol: "chart.bar.xaxis", group: .math,
                inputLabel: "Numbers · separated by spaces, commas, or newlines", example: "12, 18, 18, 21, 24, 27, 32, 36, 42, 51",
                options: [.init(id: "population", label: "Deviation", choices: ["Population", "Sample"])], height: 328)
        case .dateMath:
            return .init(title: "Date Calculator", subtitle: "Add calendar days, weekdays, months, or years with an explicit time zone", symbol: "calendar.badge.plus", group: .math,
                inputLabel: "Start date · YYYY-MM-DD or ISO 8601 with offset", example: "2026-09-12", multiline: false,
                options: [.init(id: "unit", label: "Add", choices: ["Days", "Weekdays", "Weeks", "Months", "Years", "Hours"])],
                fields: [.init(id: "amount", label: "Amount (±)", initial: "10"), .init(id: "zone", label: "Time zone", initial: "UTC")])
        case .aspectRatio:
            return .init(title: "Aspect Ratio Calculator", subtitle: "Reduce dimensions and calculate a proportional target width or height", symbol: "aspectratio", group: .design,
                inputLabel: "Source dimensions · width × height", example: "1920 x 1080", multiline: false,
                options: [.init(id: "dimension", label: "Target", choices: ["Width", "Height"])],
                fields: [.init(id: "size", label: "Pixels", initial: "1280")], height: 280)
        case .bezier:
            return .init(title: "CSS Bézier Curve", subtitle: "Tune easing curves with a live graph and copy cubic-bezier CSS", symbol: "point.topleft.down.to.point.bottomright.curvepath", group: .design,
                inputLabel: "Easing · preset name or x1, y1, x2, y2", example: "0.25, 0.1, 0.25, 1", multiline: false, height: 280)
        case .boxShadow:
            return .init(title: "CSS Box Shadow", subtitle: "Build an outer shadow with a live card and ready-to-paste CSS", symbol: "square.on.square", group: .design,
                inputLabel: "Shadow color · HEX, RGB, or HSL", example: "#00000030", multiline: false,
                fields: [.init(id: "x", label: "X px", initial: "0"), .init(id: "y", label: "Y px", initial: "8"), .init(id: "blur", label: "Blur px", initial: "24"), .init(id: "spread", label: "Spread px", initial: "0")], height: 304)
        case .textTable:
            return .init(title: "Text Table Builder", subtitle: "Turn CSV into aligned Markdown, plain text, or escaped HTML tables", symbol: "tablecells", group: .text,
                inputLabel: "CSV · first row is the table header", example: "Tool,Type,Local\nJSON Schema,Validation,Yes\nBézier Curve,Design,Yes\nSSH Inspector,Security,Yes",
                options: [.init(id: "format", label: "Output", choices: ["Markdown", "Plain text", "HTML"]), .init(id: "alignment", label: "Align", choices: ["Left", "Center", "Right"])])
        default: return nil
        }
    }
    var secondInputLabel: String? { extendedDefinition?.secondLabel ?? (self == .listSet ? "List B · one item per line" : nil) }
    var secondExample: String { extendedDefinition?.secondExample ?? (self == .listSet ? "Rust\nGo\nSwift" : self == .hmac ? "Jefe" : "") }
}

enum ExtendedUtilityEngine {
    static func run(_ kind: AdditionalUtilityKind, input: String, second: String, options: [String: String]) throws -> WorkbenchResult {
        switch kind {
        case .jsonSchema: return try JSONSchemaTool.validate(input, schema: second)
        case .jsonMerge, .jsonRedact, .jsonLines, .csvExplore, .envFile, .plist, .textTable: return try DataWorkshop.run(kind, input: input, second: second, options: options)
        case .sqlFormat: return try SQLFormatterTool.format(input, uppercase: options["keywords"] == "Uppercase", dialect: options["dialect"] ?? "PostgreSQL")
        case .httpHeaders, .cookies, .sshKey, .uuidInspect: return try ProtocolTools.run(kind, input: input, options: options)
        case .certificate: return try CertificateTool.inspect(input)
        case .bitwise, .statistics, .dateMath: return try QuantTools.run(kind, input: input, options: options)
        case .aspectRatio, .bezier, .boxShadow: return try LayoutTools.run(kind, input: input, options: options)
        default: throw UtilityError("Choose a supported tool.")
        }
    }
}
