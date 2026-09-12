import Foundation
import Security
import CryptoKit

enum CertificateTool {
    static func inspect(_ input: String) throws -> WorkbenchResult {
        guard !input.contains("PRIVATE KEY") else { throw UtilityError("Paste public PEM certificates only. Private keys are not inspected.") }
        let begin = "-----BEGIN CERTIFICATE-----", end = "-----END CERTIFICATE-----"
        var remaining = input.trimmingCharacters(in: .whitespacesAndNewlines), certificates: [String] = []
        while !remaining.isEmpty {
            try Task.checkCancellation()
            guard certificates.count < 10, remaining.hasPrefix(begin), let closing = remaining.range(of: end) else { throw UtilityError("Paste up to 10 complete PEM certificate blocks, without extra text.") }
            let payload = remaining.dropFirst(begin.count)[..<closing.lowerBound].filter { !$0.isWhitespace }
            guard payload.utf8.count <= 90_000, let data = Data(base64Encoded: String(payload)), data.count <= 65_536,
                  let certificate = SecCertificateCreateWithData(nil, data as CFData) else { throw UtilityError("Invalid certificate. Each PEM block must contain an X.509 certificate of at most 64 KB.") }
            let subject = (SecCertificateCopySubjectSummary(certificate) as String?) ?? "Unnamed certificate"
            let fingerprint = SHA256.hash(data: data).map { String(format: "%02X", $0) }.joined(separator: ":")
            var lines = ["Certificate \(certificates.count + 1) · \(subject)"]
            let dateKeys = [kSecOIDX509V1ValidityNotBefore as String, kSecOIDX509V1ValidityNotAfter as String]
            if let dates = SecCertificateCopyValues(certificate, dateKeys as CFArray, nil) as? [String: Any] {
                for (i, key) in dateKeys.enumerated() {
                    if let field = dates[key] as? [String: Any], let seconds = field[kSecPropertyKeyValue as String] as? NSNumber {
                        let iso = ISO8601DateFormatter()
                        lines.append("\(i == 0 ? "Valid from" : "Valid until")  " + iso.string(from: Date(timeIntervalSinceReferenceDate: seconds.doubleValue)))
                    }
                }
            }
            lines += ["SHA-256  \(fingerprint)", "DER size \(data.count) bytes"]
            if let serial = SecCertificateCopySerialNumberData(certificate, nil) as Data? { lines.append("Serial   " + serial.map { String(format: "%02X", $0) }.joined()) }
            if let key = SecCertificateCopyKey(certificate), let attributes = SecKeyCopyAttributes(key) as? [String: Any], let bits = attributes[kSecAttrKeySizeInBits as String] { lines.append("Public key size \(bits) bits") }
            var error: Unmanaged<CFError>?
            if let fields = SecCertificateCopyValues(certificate, nil, &error) as? [String: Any] {
                var budget = 2_000
                func describe(_ value: Any, depth: Int) -> [String] {
                    budget -= 1
                    guard budget >= 0, depth <= 16 else { return ["Additional nested details omitted."] }
                    let indent = String(repeating: "  ", count: depth)
                    if let array = value as? [Any] { return array.flatMap { describe($0, depth: depth) } }
                    if let field = value as? [String: Any] {
                        let label = field[kSecPropertyKeyLabel as String] as? String ?? "Value"
                        guard let item = field[kSecPropertyKeyValue as String] else { return [] }
                        if let seconds = item as? NSNumber, field[kSecPropertyKeyType as String] as? String == kSecPropertyTypeDate as String {
                            let iso = ISO8601DateFormatter()
                            return [indent + label + ": " + iso.string(from: Date(timeIntervalSinceReferenceDate: seconds.doubleValue))]
                        }
                        if item is [Any] || item is [String: Any] { return [indent + label + ":"] + describe(item, depth: depth + 1) }
                        if let bytes = item as? Data { return [indent + label + ": " + bytes.map { String(format: "%02X", $0) }.joined()] }
                        return [indent + label + ": " + String(describing: item)]
                    }
                    return [indent + String(describing: value)]
                }
                let preferred = [kSecOIDX509V1IssuerName as String, kSecOIDX509V1SubjectName as String]
                let order = preferred.filter { fields[$0] != nil } + fields.keys.filter { !preferred.contains($0) && !dateKeys.contains($0) }.sorted()
                for key in order { lines += describe(fields[key]!, depth: 0) }
            } else { error?.release(); lines.append("Some extended fields could not be decoded by macOS.") }
            certificates.append(lines.joined(separator: "\n"))
            remaining = String(remaining[closing.upperBound...]).trimmingCharacters(in: .whitespacesAndNewlines)
        }
        guard !certificates.isEmpty else { throw UtilityError("Paste a PEM certificate or load the example.") }
        return .init(text: certificates.joined(separator: "\n\n────────\n\n") + "\n\nInspection only. Certificate trust, hostname matching, revocation, and chain validity are not evaluated. No network requests are made.", status: "\(certificates.count) \(certificates.count == 1 ? "certificate" : "certificates") · local inspection · trust not evaluated")
    }
}
