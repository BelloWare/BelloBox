import Foundation
import CryptoKit

enum SecurityUtility {
    static func permissionBits(_ input: String) throws -> Int {
        let raw = input.trimmingCharacters(in: .whitespacesAndNewlines)
        if (3...4).contains(raw.count), raw.utf8.allSatisfy({ (48...55).contains($0) }), let number = Int(raw, radix: 8) { return number }
        let chars = Array(raw.count == 10 && ["-", "d", "l"].contains(String(raw.prefix(1))) ? String(raw.dropFirst()) : raw)
        guard chars.count == 9 else { throw UtilityError("Enter 3–4 octal digits (755, 4755) or nine permission characters (rwxr-xr-x).") }
        var bits = 0
        for i in chars.indices {
            let expected: Character = i % 3 == 0 ? "r" : i % 3 == 1 ? "w" : "x"
            let char = chars[i]
            if char == expected { bits |= 1 << (8 - i) }
            else if i % 3 == 2 && ((i < 8 && (char == "s" || char == "S")) || (i == 8 && (char == "t" || char == "T"))) {
                bits |= i == 2 ? 0o4000 : i == 5 ? 0o2000 : 0o1000
                if char == "s" || char == "t" { bits |= 1 << (8 - i) }
            } else if char != "-" { throw UtilityError("Invalid permission character at position \(i + 1).") }
        }
        return bits
    }
    static func symbolic(_ bits: Int) -> String {
        var chars = Array("---------")
        for i in 0..<9 where bits & (1 << (8 - i)) != 0 { chars[i] = i % 3 == 0 ? "r" : i % 3 == 1 ? "w" : "x" }
        for (index, special) in [(2, 0o4000), (5, 0o2000), (8, 0o1000)] where bits & special != 0 {
            chars[index] = index == 8 ? (chars[index] == "x" ? "t" : "T") : (chars[index] == "x" ? "s" : "S")
        }
        return String(chars)
    }
    static func run(_ kind: AdditionalUtilityKind, input: String, second: String, options: [String: String]) throws -> WorkbenchResult {
        if kind == .subnet { return try subnet(input) }
        if kind == .chmod {
            let bits = try permissionBits(input)
            let octal = String(format: bits > 0o777 ? "%04o" : "%03o", bits)
            return WorkbenchResult(text: "chmod \(octal) 'path/to/file'", status: "Permissions preview · no files are changed", visual: .permissions(bits))
        }
        guard !second.isEmpty else { throw UtilityError("Enter a secret key. It stays in this tool's temporary draft and is never saved.") }
        let key: Data
        if options["keyFormat"] == "Hex" {
            guard second.utf8.count <= 8_192, second.utf8.count % 2 == 0, second.utf8.allSatisfy({ (48...57).contains($0) || (65...70).contains($0) || (97...102).contains($0) }) else { throw UtilityError("A hex key needs an even number of hex digits (up to 8,192), without spaces or a prefix.") }
            let bytes = Array(second.utf8)
            key = Data(stride(from: 0, to: bytes.count, by: 2).map { UInt8(String(decoding: bytes[$0...($0 + 1)], as: UTF8.self), radix: 16)! })
        } else {
            guard second.utf8.count <= 4_096 else { throw UtilityError("Use a key of at most 4 KB.") }
            key = Data(second.utf8)
        }
        let symmetric = SymmetricKey(data: key), message = Data(input.utf8), digest: Data
        if options["algorithm"] == "SHA-512" { digest = Data(HMAC<SHA512>.authenticationCode(for: message, using: symmetric)) }
        else { digest = Data(HMAC<SHA256>.authenticationCode(for: message, using: symmetric)) }
        return WorkbenchResult(text: options["encoding"] == "Base64" ? digest.base64EncodedString() : digest.map { String(format: "%02x", $0) }.joined(), status: "HMAC-\(options["algorithm"]!) · UTF-8 message · key never persisted")
    }
    static func subnet(_ input: String) throws -> WorkbenchResult {
        let raw = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard raw.utf8.count <= 64 else { throw UtilityError("Enter one IPv4 CIDR, such as 192.168.1.42/24.") }
        let parts = raw.split(separator: "/", omittingEmptySubsequences: false)
        guard parts.count == 2, let prefix = JSONPointerTool.index(String(parts[1])), (0...32).contains(prefix) else { throw UtilityError("Use IPv4/prefix with a prefix from 0 to 32.") }
        let octets = parts[0].split(separator: ".", omittingEmptySubsequences: false)
        guard octets.count == 4 else { throw UtilityError("IPv4 needs four decimal octets.") }
        var address: UInt32 = 0
        for octet in octets {
            guard let n = JSONPointerTool.index(String(octet)), n <= 255 else { throw UtilityError("IPv4 octets must be 0–255, with no leading zeros.") }
            address = (address << 8) | UInt32(n)
        }
        let mask: UInt32 = prefix == 0 ? 0 : UInt32.max << (32 - prefix)
        let network = address & mask, broadcast = network | ~mask, count = UInt64(1) << (32 - prefix)
        let first = prefix >= 31 ? network : network + 1, last = prefix >= 31 ? broadcast : broadcast - 1
        func ip(_ n: UInt32) -> String { [24, 16, 8, 0].map { String((n >> $0) & 255) }.joined(separator: ".") }
        let note = prefix == 31 ? "Point-to-point /31: both addresses are usable; no directed broadcast." : prefix == 32 ? "Single-host /32: one address; no directed broadcast." : "Usable range excludes network and broadcast addresses."
        return WorkbenchResult(text: "Network      \(ip(network))/\(prefix)\nNetmask      \(ip(mask))\nWildcard     \(ip(~mask))\nLast address \(ip(broadcast))\nBroadcast    \(prefix >= 31 ? "Not applicable" : ip(broadcast))\nFirst host   \(ip(first))\nLast host    \(ip(last))\nAddresses    \(count)\nUsable hosts \(prefix >= 31 ? count : count - 2)\n\n\(note)", status: "IPv4 subnet · calculated locally")
    }
}
