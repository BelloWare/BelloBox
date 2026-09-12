import Foundation

struct StatisticsVisual {
    let bins: [Int]
    let minimum: Double, maximum: Double, mean: Double, median: Double
    let count: Int
}

enum QuantTools {
    static func run(_ kind: AdditionalUtilityKind, input: String, options: [String: String]) throws -> WorkbenchResult {
        switch kind {
        case .bitwise: return try bitwise(input, options: options)
        case .statistics: return try statistics(input, sample: options["population"] == "Sample")
        case .dateMath: return try date(input, options: options)
        default: throw UtilityError("Choose a numeric tool.")
        }
    }
    static func word(_ input: String, width: Int) throws -> UInt64 {
        var token = input.trimmingCharacters(in: .whitespacesAndNewlines).lowercased(), negative = false
        if token.hasPrefix("-") { negative = true; token.removeFirst() }
        else if token.hasPrefix("+") { token.removeFirst() }
        let base: Int
        if token.hasPrefix("0x") { base = 16; token.removeFirst(2) }
        else if token.hasPrefix("0b") { base = 2; token.removeFirst(2) }
        else { base = 10 }
        guard !token.isEmpty, token.utf8.count <= 64, token.utf8.allSatisfy({ (48...57).contains($0) || base == 16 && (97...102).contains($0) }), let magnitude = UInt64(token, radix: base) else { throw UtilityError("Use a whole decimal, 0x hexadecimal, or 0b binary number within 64 bits.") }
        let mask = width == 64 ? UInt64.max : (UInt64(1) << width) - 1
        guard magnitude <= (negative ? UInt64(1) << (width - 1) : mask) else { throw UtilityError("Operand does not fit \(width) bits. Choose a wider word.") }
        return (negative ? 0 &- magnitude : magnitude) & mask
    }
    static func bitwise(_ input: String, options: [String: String]) throws -> WorkbenchResult {
        let width = Int(options["width"] ?? "8") ?? 8
        guard [8,16,32,64].contains(width) else { throw UtilityError("Choose 8, 16, 32, or 64 bits.") }
        let a = try word(input, width: width), op = options["operation"] ?? "AND", mask = width == 64 ? UInt64.max : (UInt64(1) << width) - 1
        let bText = options["operand"] ?? "0", result: UInt64
        switch op {
        case "NOT": result = ~a & mask
        case "AND": result = a & (try word(bText, width: width))
        case "OR": result = a | (try word(bText, width: width))
        case "XOR": result = a ^ (try word(bText, width: width))
        case "Left shift", "Right shift", "Rotate left", "Rotate right":
            guard let count = Int(bText), (0..<width).contains(count) else { throw UtilityError("Enter a decimal shift count from 0 to \(width - 1) in Operand B / shift.") }
            if count == 0 { result = a }
            else if op == "Left shift" { result = (a << count) & mask }
            else if op == "Right shift" { result = a >> count }
            else if op == "Rotate left" { result = ((a << count) | (a >> (width - count))) & mask }
            else { result = ((a >> count) | (a << (width - count))) & mask }
        default: throw UtilityError("Choose a bitwise operation.")
        }
        let raw = String(result, radix: 2), bits = String(repeating: "0", count: width - raw.count) + raw
        let signed = width == 64 ? String(Int64(bitPattern: result)) : result & (UInt64(1) << (width - 1)) != 0 ? String(Int64(result) - (Int64(1) << width)) : String(result)
        let text = "Unsigned  \(result)\nSigned    \(signed)\nHex       0x\(String(result, radix: 16).uppercased())\nBinary    \(bits)\nSet bits  \(result.nonzeroBitCount) / \(width)"
        return .init(text: text, status: "\(op) · \(width)-bit word · logical shifts · overflow bits discarded", visual: .bits(bits, signed))
    }
    static func statistics(_ input: String, sample: Bool) throws -> WorkbenchResult {
        let tokens = input.split { $0 == "," || $0.isWhitespace }
        guard !tokens.isEmpty, tokens.count <= 20_000 else { throw UtilityError("Enter 1–20,000 numbers separated by spaces, commas, or newlines.") }
        var numbers: [Double] = [], mean = 0.0, m2 = 0.0, sum = 0.0, compensation = 0.0
        for (i, token) in tokens.enumerated() {
            try Task.checkCancellation()
            guard let value = Double(token), value.isFinite, abs(value) <= 1e150 else { throw UtilityError("Value \(i + 1) must be a finite number between −1e150 and 1e150.") }
            numbers.append(value)
            let delta = value - mean
            mean += delta / Double(i + 1); m2 += delta * (value - mean)
            let adjusted = value - compensation, next = sum + adjusted
            compensation = (next - sum) - adjusted; sum = next
        }
        let sorted = numbers.sorted(), n = sorted.count, low = sorted[0], high = sorted[n - 1]
        func percentile(_ p: Double) -> Double {
            let index = Double(n - 1) * p, lower = Int(index), weight = index - Double(lower)
            return sorted[lower] * (1 - weight) + sorted[min(lower + 1, n - 1)] * weight
        }
        let median = percentile(0.5), variance = sample && n == 1 ? nil : max(0, m2 / Double(sample ? n - 1 : n))
        let deviation = variance.map { MathTool.display(sqrt($0)) } ?? "Needs at least 2 values"
        var frequencies: [Double: Int] = [:]
        numbers.forEach { frequencies[$0, default: 0] += 1 }
        let frequency = frequencies.values.max() ?? 0, modes = frequencies.filter { $0.value == frequency }.keys.sorted()
        let mode = frequency <= 1 ? "No repeated values" : modes.prefix(10).map(MathTool.display).joined(separator: ", ") + (modes.count > 10 ? " (\(modes.count) tied values)" : "") + " · \(frequency) occurrences"
        let binCount = low == high ? 1 : min(12, max(5, Int(sqrt(Double(n)))))
        var bins = Array(repeating: 0, count: binCount)
        for number in numbers {
            let index = low == high ? 0 : min(binCount - 1, Int((number - low) / (high - low) * Double(binCount)))
            bins[max(0, index)] += 1
        }
        let text = "Count       \(n)\nSum         \(MathTool.display(sum))\nMean        \(MathTool.display(mean))\nMedian      \(MathTool.display(median))\nMinimum     \(MathTool.display(low))\nMaximum     \(MathTool.display(high))\nRange       \(MathTool.display(high - low))\nP25         \(MathTool.display(percentile(0.25)))\nP75         \(MathTool.display(percentile(0.75)))\nP90         \(MathTool.display(percentile(0.9)))\nP95         \(MathTool.display(percentile(0.95)))\nStd. dev.   \(deviation)\nMode        \(mode)"
        return .init(text: text, status: "\(n) values · \(sample ? "sample" : "population") deviation · interpolated percentiles", visual: .statistics(.init(bins: bins, minimum: low, maximum: high, mean: mean, median: median, count: n)))
    }
    static func date(_ input: String, options: [String: String]) throws -> WorkbenchResult {
        let zoneName = options["zone"] ?? "UTC"
        guard let zone = TimeZone(identifier: zoneName) else { throw UtilityError("Use an IANA time zone, such as UTC, Asia/Singapore, or America/New_York.") }
        guard let amount = Int(options["amount"] ?? "10"), (-10_000...10_000).contains(amount) else { throw UtilityError("Enter a whole amount from −10,000 to 10,000.") }
        let start = try parseDate(input, zone: zone)
        var calendar = Calendar(identifier: .gregorian); calendar.timeZone = zone
        let unit = options["unit"] ?? "Days"
        let result: Date
        if unit == "Weekdays" {
            var current = start, remaining = abs(amount), direction = amount < 0 ? -1 : 1
            while remaining > 0 {
                try Task.checkCancellation()
                guard let next = calendar.date(byAdding: .day, value: direction, to: current) else { throw UtilityError("The resulting date is outside the supported range.") }
                current = next
                if ![1, 7].contains(calendar.component(.weekday, from: current)) { remaining -= 1 }
            }
            result = current
        } else {
            let components: [String: Calendar.Component] = ["Days": .day, "Weeks": .weekOfYear, "Months": .month, "Years": .year, "Hours": .hour]
            guard let component = components[unit], let next = calendar.date(byAdding: component, value: amount, to: start) else { throw UtilityError("Choose a valid date unit.") }
            result = next
        }
        guard calendar.component(.era, from: result) == 1, (1...9999).contains(calendar.component(.year, from: result)) else { throw UtilityError("Keep dates within Gregorian years 1–9999.") }
        let formatter = DateFormatter(); formatter.locale = Locale(identifier: "en_US_POSIX"); formatter.calendar = calendar; formatter.timeZone = zone; formatter.dateFormat = "EEEE, yyyy-MM-dd HH:mm:ss XXXXX"
        let utc = ISO8601DateFormatter(); utc.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let seconds = result.timeIntervalSince(start)
        let note = unit == "Weekdays" ? "Weekdays exclude Saturday/Sunday; public holidays are not excluded." : "Calendar units use the selected zone; month/year additions clamp to its last valid day."
        return .init(text: "\(formatter.string(from: result))\n\(zone.identifier)\n\nUTC         \(utc.string(from: result))\nElapsed     \(MathTool.display(seconds)) seconds\nHours       \(MathTool.display(seconds / 3600))\n\n\(note)", status: "\(amount >= 0 ? "+" : "")\(amount) \(unit.lowercased()) · \(zone.identifier)")
    }
    static func parseDate(_ input: String, zone: TimeZone) throws -> Date {
        let text = input.trimmingCharacters(in: .whitespacesAndNewlines)
        let formatter = DateFormatter(); formatter.locale = Locale(identifier: "en_US_POSIX"); formatter.calendar = Calendar(identifier: .gregorian); formatter.timeZone = zone; formatter.dateFormat = "yyyy-MM-dd"; formatter.isLenient = false
        if text.count == 10, let date = formatter.date(from: text), formatter.string(from: date) == text, !text.hasPrefix("0000") { return date }
        let pattern = #"^([0-9]{4}-[0-9]{2}-[0-9]{2})T([0-9]{2}):([0-9]{2}):([0-9]{2})(?:\.[0-9]{1,9})?(Z|[+-][0-9]{2}:[0-9]{2})$"#
        let regex = try NSRegularExpression(pattern: pattern)
        guard let match = regex.firstMatch(in: text, range: NSRange(text.startIndex..., in: text)) else { throw UtilityError("Use YYYY-MM-DD or a complete ISO 8601 timestamp with Z or a numeric offset.") }
        func part(_ i: Int) -> String { (text as NSString).substring(with: match.range(at: i)) }
        let offset = part(5)
        if offset != "Z" {
            let pieces = offset.dropFirst().split(separator: ":")
            guard Int(pieces[0])! <= 23, Int(pieces[1])! <= 59 else { throw UtilityError("Invalid timestamp offset.") }
        }
        formatter.timeZone = TimeZone(secondsFromGMT: 0)
        guard let day = formatter.date(from: part(1)), formatter.string(from: day) == part(1), !part(1).hasPrefix("0000"), Int(part(2))! < 24, Int(part(3))! < 60, Int(part(4))! < 60 else { throw UtilityError("Invalid calendar date or time.") }
        let iso = ISO8601DateFormatter(); iso.formatOptions = text.contains(".") ? [.withInternetDateTime, .withFractionalSeconds] : [.withInternetDateTime]
        guard let date = iso.date(from: text) else { throw UtilityError("Invalid ISO 8601 timestamp.") }
        return date
    }
}
