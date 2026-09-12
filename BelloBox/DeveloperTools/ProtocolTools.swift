import Foundation
import CryptoKit

enum ProtocolTools {
    static func run(_ kind: AdditionalUtilityKind, input: String, options: [String: String]) throws -> WorkbenchResult {
        switch kind {
        case .httpHeaders: return try headers(input)
        case .cookies: return try cookies(input, request: options["mode"] == "Cookie")
        case .sshKey: return try ssh(input)
        case .uuidInspect: return try uuid(input)
        default: throw UtilityError("Choose an inspector.")
        }
    }
    static func token(_ text: String) -> Bool {
        !text.isEmpty && text.utf8.allSatisfy { (48...57).contains($0) || (65...90).contains($0) || (97...122).contains($0) || "!#$%&'*+-.^_`|~".utf8.contains($0) }
    }
    static func headers(_ input: String) throws -> WorkbenchResult {
        let lines = DataWorkshop.normalizedLines(input)
        guard lines.count <= 5_000 else { throw UtilityError("Inspect up to 5,000 header lines.") }
        var startLine: String?, fields: [DeveloperJSON] = [], counts: [String: Int] = [:], body = false, ended = false
        for (i, line) in lines.enumerated() {
            try Task.checkCancellation()
            if line.isEmpty { if !fields.isEmpty || startLine != nil { ended = true }; continue }
            if ended { body = true; continue }
            if i == 0 && (line.hasPrefix("HTTP/") || line.range(of: #"^[A-Z]+ \S+ HTTP/[0-9.]+$"#, options: .regularExpression) != nil) { startLine = line; continue }
            guard !line.hasPrefix(" "), !line.hasPrefix("\t") else { throw UtilityError("Line \(i + 1) uses obsolete folded headers. Put each field on its own line.") }
            let search = line.hasPrefix(":") ? line.dropFirst() : Substring(line)
            guard let colon = search.firstIndex(of: ":") else { throw UtilityError("Line \(i + 1) needs a header name followed by a colon.") }
            let name = String(line[..<colon]), checkName = name.hasPrefix(":") ? String(name.dropFirst()) : name
            guard token(checkName) else { throw UtilityError("Invalid header name on line \(i + 1).") }
            let value = line[line.index(after: colon)...].trimmingCharacters(in: .whitespaces)
            guard !value.unicodeScalars.contains(where: { $0.value < 32 && $0.value != 9 || $0.value == 127 }) else { throw UtilityError("Header values cannot contain control characters.") }
            fields.append(.object(["name": .string(name), "value": .string(value)])); counts[name.lowercased(), default: 0] += 1
        }
        guard !fields.isEmpty || startLine != nil else { throw UtilityError("Paste a header block to inspect.") }
        var object: [String: DeveloperJSON] = ["headers": .array(fields)]
        if let startLine { object["startLine"] = .string(startLine) }
        let duplicates = counts.values.filter { $0 > 1 }.count
        return .init(text: DeveloperJSON.object(object).formatted(), status: "\(fields.count) fields · \(duplicates) repeated names preserved" + (body ? " · body omitted" : ""))
    }
    static func cookies(_ input: String, request: Bool) throws -> WorkbenchResult {
        let lines = DataWorkshop.normalizedLines(input).filter { !$0.trimmingCharacters(in: .whitespaces).isEmpty }
        guard lines.count <= 1_000 else { throw UtilityError("Inspect up to 1,000 cookie lines.") }
        var records: [DeveloperJSON] = []
        func pair(_ text: String) throws -> (String, String) {
            guard let equals = text.firstIndex(of: "=") else { throw UtilityError("A cookie needs name=value.") }
            let name = text[..<equals].trimmingCharacters(in: .whitespaces), value = text[text.index(after: equals)...].trimmingCharacters(in: .whitespaces)
            guard token(name), !value.contains(","), !value.unicodeScalars.contains(where: { $0.value < 32 || $0.value == 127 }) else { throw UtilityError("Invalid cookie pair. Put separate Set-Cookie fields on separate lines; do not combine them with commas.") }
            return (name, value)
        }
        for raw in lines {
            try Task.checkCancellation()
            let prefix = request ? "cookie:" : "set-cookie:", line = raw.lowercased().hasPrefix(prefix) ? String(raw.dropFirst(prefix.count)) : raw
            let pieces = line.split(separator: ";", omittingEmptySubsequences: false).map { $0.trimmingCharacters(in: .whitespaces) }
            if request {
                for piece in pieces where !piece.isEmpty { let (name, value) = try pair(piece); records.append(.object(["name": .string(name), "value": .string(value)])) }
            } else {
                guard let first = pieces.first else { continue }
                let (name, value) = try pair(first)
                var attributes: [DeveloperJSON] = []
                for piece in pieces.dropFirst() where !piece.isEmpty {
                    guard piece.range(of: #",\s*[!#$%&'*+.^_`|~0-9A-Za-z-]+\s*="#, options: .regularExpression) == nil else { throw UtilityError("Put each Set-Cookie field on a separate line; a combined cookie header is ambiguous.") }
                    let split = piece.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
                    let name = split[0].trimmingCharacters(in: .whitespaces)
                    guard token(name) else { throw UtilityError("Invalid cookie attribute name.") }
                    let value: DeveloperJSON = split.count == 2 ? .string(split[1].trimmingCharacters(in: .whitespaces)) : .bool(true)
                    attributes.append(.object(["name": .string(name), "value": value]))
                }
                records.append(.object(["name": .string(name), "value": .string(value), "attributes": .array(attributes)]))
            }
            guard records.count <= 5_000 else { throw UtilityError("Use up to 5,000 cookies.") }
        }
        return .init(text: DeveloperJSON.array(records).formatted(), status: "\(records.count) \(records.count == 1 ? "cookie" : "cookies") · literal values and duplicates preserved · browser storage untouched")
    }
    struct SSHReader {
        let bytes: [UInt8]
        var offset = 0
        mutating func field() throws -> [UInt8] {
            guard offset + 4 <= bytes.count else { throw UtilityError("Truncated SSH public key.") }
            let length = bytes[offset..<(offset + 4)].reduce(UInt32(0)) { $0 << 8 | UInt32($1) }; offset += 4
            guard length <= 16_384, Int(length) <= bytes.count - offset else { throw UtilityError("Invalid SSH field length.") }
            let field = Array(bytes[offset..<(offset + Int(length))]); offset += Int(length); return field
        }
        mutating func string() throws -> String {
            guard let string = String(bytes: try field(), encoding: .utf8) else { throw UtilityError("Invalid UTF-8 SSH key type.") }
            return string
        }
    }
    static func ssh(_ input: String) throws -> WorkbenchResult {
        guard input.utf8.count <= 32_768, !input.contains("PRIVATE KEY") else { throw UtilityError("Paste one OpenSSH public key (up to 32 KB), not a private key or authorized_keys options.") }
        let parts = input.trimmingCharacters(in: .whitespacesAndNewlines).split(maxSplits: 2, whereSeparator: \.isWhitespace)
        guard parts.count >= 2, let data = Data(base64Encoded: String(parts[1])), data.count <= 16_384 else { throw UtilityError("Use an OpenSSH public key: key-type Base64 [comment].") }
        var reader = SSHReader(bytes: Array(data))
        let type = try reader.string()
        guard type == parts[0] else { throw UtilityError("The SSH key's type does not match its encoded payload.") }
        let detail: String
        switch type {
        case "ssh-ed25519":
            let key = try reader.field()
            guard key.count == 32 else { throw UtilityError("An Ed25519 public key must contain 32 key bytes.") }
            detail = "256-bit Ed25519 public key"
        case "ssh-rsa":
            func integer(_ value: [UInt8]) throws -> [UInt8] {
                guard !value.isEmpty, value[0] & 0x80 == 0, value.count == 1 || value[0] != 0 || value[1] & 0x80 != 0 else { throw UtilityError("RSA fields must be positive, minimally encoded SSH integers.") }
                let bytes = Array(value.drop(while: { $0 == 0 }))
                guard !bytes.isEmpty else { throw UtilityError("RSA exponent and modulus must be positive.") }; return bytes
            }
            let exponent = try integer(reader.field()), modulus = try integer(reader.field())
            guard exponent.count <= 4 else { throw UtilityError("Inspect RSA keys with exponents up to 32 bits.") }
            let e = exponent.reduce(UInt64(0)) { $0 << 8 | UInt64($1) }
            guard e >= 3, e % 2 == 1 else { throw UtilityError("RSA exponent must be an odd integer of at least 3.") }
            let bits = (modulus.count - 1) * 8 + (8 - modulus[0].leadingZeroBitCount)
            detail = "\(bits)-bit RSA modulus · exponent \(e)"
        case "ecdsa-sha2-nistp256", "ecdsa-sha2-nistp384", "ecdsa-sha2-nistp521":
            let curve = try reader.string(), point = Data(try reader.field())
            guard type == "ecdsa-sha2-" + curve else { throw UtilityError("ECDSA curve and key type disagree.") }
            do {
                if curve == "nistp256" { _ = try P256.Signing.PublicKey(x963Representation: point) }
                else if curve == "nistp384" { _ = try P384.Signing.PublicKey(x963Representation: point) }
                else { _ = try P521.Signing.PublicKey(x963Representation: point) }
            } catch { throw UtilityError("Invalid ECDSA public point.") }
            detail = "ECDSA · \(curve)"
        default: throw UtilityError("Supported public keys: ssh-ed25519, ssh-rsa, and ecdsa-sha2-nistp256/384/521.")
        }
        guard reader.offset == reader.bytes.count else { throw UtilityError("Unexpected bytes after the SSH public key.") }
        let fingerprint = Data(SHA256.hash(data: data)).base64EncodedString().replacingOccurrences(of: "=", with: "")
        let comment = parts.count == 3 ? String(parts[2]) : "None"
        return .init(text: "Type         \(type)\nKey          \(detail)\nFingerprint  SHA256:\(fingerprint)\nComment      \(comment)\n\nFingerprint identifies these public-key bytes; identity and ownership are not verified.", status: "Public key inspected locally · SHA-256 fingerprint")
    }
    static func uuid(_ input: String) throws -> WorkbenchResult {
        var text = input.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if text.hasPrefix("urn:uuid:") { text = String(text.dropFirst(9)) }
        if text.hasPrefix("{"), text.hasSuffix("}") { text = String(text.dropFirst().dropLast()) }
        if text.count == 36 {
            let chars = Array(text)
            guard [8,13,18,23].allSatisfy({ chars[$0] == "-" }) else { throw UtilityError("Invalid UUID hyphen positions.") }
            text = text.replacingOccurrences(of: "-", with: "")
        }
        guard text.utf8.count == 32, text.utf8.allSatisfy({ (48...57).contains($0) || (97...102).contains($0) }) else { throw UtilityError("Enter a 32-digit UUID, optionally hyphenated or prefixed with urn:uuid:.") }
        let chars = Array(text), bytes = stride(from: 0, to: 32, by: 2).map { UInt8(String(chars[$0..<($0 + 2)]), radix: 16)! }
        func field(_ range: Range<Int>) -> UInt64 { bytes[range].reduce(UInt64(0)) { $0 << 8 | UInt64($1) } }
        let canonical = [0..<8, 8..<12, 12..<16, 16..<20, 20..<32].map { String(chars[$0]) }.joined(separator: "-")
        let version = bytes[6] >> 4, variant = bytes[8] & 0x80 == 0 ? "NCS reserved" : bytes[8] & 0xC0 == 0x80 ? "RFC 9562" : bytes[8] & 0xE0 == 0xC0 ? "Microsoft reserved" : "Future reserved"
        let special = bytes.allSatisfy { $0 == 0 } ? "Nil UUID" : bytes.allSatisfy { $0 == 255 } ? "Max UUID" : nil
        let descriptions: [UInt8: String] = [1: "Gregorian time", 2: "DCE security", 3: "Name-based MD5", 4: "Random", 5: "Name-based SHA-1", 6: "Reordered Gregorian time", 7: "Unix millisecond time", 8: "Custom"]
        var output = "\(canonical)\n\nVersion  \(special ?? "\(version) · \(descriptions[version] ?? "Reserved")")\nVariant  \(variant)\nBytes    \(bytes.map { String(format: "%02X", $0) }.joined(separator: " "))"
        if variant == "RFC 9562" && special == nil {
            var seconds: Double?
            if version == 7 { let milliseconds = field(0..<6); seconds = Double(milliseconds) / 1000; output += "\nUnix ms  \(milliseconds)" }
            if version == 1 || version == 6 {
                let ticks = version == 1 ? (UInt64(bytes[6] & 15) << 56 | UInt64(bytes[7]) << 48 | field(4..<6) << 32 | field(0..<4)) : (field(0..<6) << 12 | UInt64(bytes[6] & 15) << 8 | UInt64(bytes[7]))
                seconds = Double(Int64(ticks) - 122_192_928_000_000_000) / 10_000_000
                output += "\nClock sequence  \((UInt16(bytes[8] & 63) << 8) | UInt16(bytes[9]))\nNode identifier \(bytes[10..<16].map { String(format: "%02x", $0) }.joined(separator: ":"))"
            }
            if let seconds { let formatter = ISO8601DateFormatter(); formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]; output += "\nUTC      \(formatter.string(from: Date(timeIntervalSince1970: seconds)))" }
        }
        return .init(text: output, status: special ?? "UUID v\(version) · \(variant) · timestamp is metadata, not proof of creation")
    }
}
