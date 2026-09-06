import Foundation

struct HTTPRequestDraft: Equatable {
    var method = "GET"
    var url = ""
    var headers = ""
    var body = ""

    func request() throws -> URLRequest {
        guard let parsed = URL(string: url), ["http", "https"].contains(parsed.scheme?.lowercased() ?? ""), parsed.host != nil else {
            throw UtilityError("Enter a complete http:// or https:// request URL.")
        }
        let method = method.uppercased().trimmingCharacters(in: .whitespacesAndNewlines)
        guard !method.isEmpty, method.utf8.allSatisfy({ (65...90).contains($0) || $0 == 45 }) else { throw UtilityError("Enter a valid HTTP method.") }
        try UtilityLimits.check(body)
        var request = URLRequest(url: parsed)
        request.httpMethod = method
        request.timeoutInterval = 30
        for line in headers.components(separatedBy: "\n") where !line.trimmingCharacters(in: .whitespaces).isEmpty {
            guard !line.contains("\r"), let colon = line.firstIndex(of: ":") else { throw UtilityError("Use one header per line: Name: value.") }
            let name = String(line[..<colon]).trimmingCharacters(in: .whitespaces)
            let value = String(line[line.index(after: colon)...]).trimmingCharacters(in: .whitespaces)
            let tokens = "!#$%&'*+-.^_`|~"
            guard !name.isEmpty, name.utf8.allSatisfy({ (65...90).contains($0) || (97...122).contains($0) || (48...57).contains($0) || tokens.utf8.contains($0) }) else { throw UtilityError("Invalid HTTP header name.") }
            guard value.unicodeScalars.allSatisfy({ ($0.value >= 32 && $0.value != 127) || $0.value == 9 }) else { throw UtilityError("HTTP header values cannot contain control characters.") }
            // Let URLSession determine framing for the actual edited body.
            guard !["content-length", "transfer-encoding"].contains(name.lowercased()) else { continue }
            request.addValue(value, forHTTPHeaderField: name)
        }
        if !body.isEmpty { request.httpBody = Data(body.utf8) }
        return request
    }
}

enum CurlImporter {
    static func parse(_ command: String) throws -> HTTPRequestDraft {
        try UtilityLimits.check(command)
        let tokens = try tokenize(command)
        let longOptions = ["--request", "--header", "--data", "--data-binary", "--data-raw", "--json", "--url", "--user", "--cookie"]
        guard tokens.first == "curl" else { throw UtilityError("Paste a cURL command beginning with curl.") }
        var draft = HTTPRequestDraft(), index = 1, explicitMethod = false, url: String?, headerLines: [String] = [], bodyParts: [String] = []
        var get = false, json = false, attachedValue: String?
        func value(_ option: String) throws -> String {
            if let attached = attachedValue { attachedValue = nil; return attached }
            guard index < tokens.count else { throw UtilityError("Missing value for \(option).") }
            let result = tokens[index]; index += 1; return result
        }
        while index < tokens.count {
            var token = tokens[index]; index += 1
            // Split attached values only when reading an option. A quoted body
            // such as '-dhello' must remain literal when value() consumes it.
            if let equals = token.firstIndex(of: "="), longOptions.contains(String(token[..<equals])) {
                attachedValue = String(token[token.index(after: equals)...]); token = String(token[..<equals])
            } else if token.count > 2, ["-X", "-H", "-d", "-u", "-b"].contains(String(token.prefix(2))) {
                attachedValue = String(token.dropFirst(2)); token = String(token.prefix(2))
            }
            switch token {
            case "-X", "--request": draft.method = try value(token).uppercased(); explicitMethod = true
            case "-H", "--header":
                let header = try value(token)
                guard !header.contains("\n"), !header.contains("\r") else { throw UtilityError("A header cannot contain a newline.") }
                headerLines.append(header)
            case "-d", "--data", "--data-binary", "--data-raw", "--json":
                let body = try value(token)
                if token != "--data-raw" && body.hasPrefix("@") { throw UtilityError("File uploads are not imported. Paste the body into the request editor.") }
                if (json && token != "--json") || (token == "--json" && !json && !bodyParts.isEmpty) { throw UtilityError("Use either JSON data or form data in a single imported request.") }
                bodyParts.append(body)
                if token == "--json" { json = true }
            case "--url":
                guard url == nil else { throw UtilityError("Import one request URL at a time.") }
                url = try value(token)
            case "-G", "--get": get = true
            case "-I", "--head": draft.method = "HEAD"; explicitMethod = true
            case "--compressed", "-s", "--silent", "-S", "--show-error", "--globoff", "-g": break
            case "-b", "--cookie":
                let cookie = try value(token)
                guard cookie.contains("="), !cookie.contains("\n"), !cookie.contains("\r") else { throw UtilityError("Import inline cookies as name=value; cookie files are not read.") }
                headerLines.append("Cookie: " + cookie)
            case "-u", "--user":
                let credentials = try value(token)
                headerLines.append("Authorization: Basic " + Data(credentials.utf8).base64EncodedString())
            default:
                if token.hasPrefix("-") { throw UtilityError("Unsupported cURL option: \(token). Remove it or configure the request in the editor.") }
                guard url == nil else { throw UtilityError("Import one request URL at a time.") }
                url = token
            }
        }
        guard let url else { throw UtilityError("The cURL command has no URL.") }
        draft.url = url
        draft.body = bodyParts.joined(separator: json ? "" : "&")
        if !bodyParts.isEmpty {
            if get {
                guard var parts = URLComponents(string: url) else { throw UtilityError("Invalid URL.") }
                let query = [parts.percentEncodedQuery, draft.body].compactMap { $0 }.filter { !$0.isEmpty }.joined(separator: "&")
                // Validate before assigning percentEncodedQuery, which otherwise traps on invalid escapes.
                var allowed = CharacterSet.urlQueryAllowed
                allowed.insert(charactersIn: "%")
                guard query.addingPercentEncoding(withAllowedCharacters: allowed) == query,
                      query.removingPercentEncoding != nil else { throw UtilityError("GET data must be URL-encoded before importing.") }
                parts.percentEncodedQuery = query
                draft.url = parts.string ?? url
                draft.body = ""
            } else if !explicitMethod { draft.method = "POST" }
            if !headerLines.contains(where: { $0.lowercased().hasPrefix("content-type:") }) && !get {
                headerLines.append(json ? "Content-Type: application/json" : "Content-Type: application/x-www-form-urlencoded")
            }
        }
        if json && !headerLines.contains(where: { $0.lowercased().hasPrefix("accept:") }) { headerLines.append("Accept: application/json") }
        draft.headers = headerLines.joined(separator: "\n")
        _ = try draft.request()
        return draft
    }

    static func tokenize(_ command: String) throws -> [String] {
        var tokens: [String] = [], token = "", quote: Character?, escaped = false, escapedInDoubleQuotes = false, started = false
        for character in command {
            if escaped {
                if character != "\n" && character != "\r\n" {
                    if escapedInDoubleQuotes && !"$`\"\\".contains(character) { token.append("\\") }
                    token.append(character); started = true
                }
                escaped = false; continue
            }
            if character == "\\" && quote != "'" { escaped = true; escapedInDoubleQuotes = quote == "\""; continue }
            if let current = quote {
                if current == "\"" && (character == "$" || character == "`") { throw UtilityError("Shell expansion is not imported. Use single quotes or escape the literal character.") }
                if character == current { quote = nil } else { token.append(character) }
                started = true; continue
            }
            if character == "'" || character == "\"" { quote = character; started = true }
            else if character.isWhitespace {
                if started { tokens.append(token); token = ""; started = false }
            } else {
                guard !";|&<>`$".contains(character) else { throw UtilityError("Shell operators and expansion are not imported. Quote URLs and use literal request values.") }
                token.append(character); started = true
            }
        }
        guard quote == nil, !escaped else { throw UtilityError("The cURL command has an unclosed quote or trailing escape.") }
        if started { tokens.append(token) }
        return tokens
    }
}

struct HTTPInspectionResult {
    let status: String
    let headers: String
    let body: String
    var text: String { status + "\n\n" + headers + "\n\n" + body }
}
final class RequestRedirectPolicy: NSObject, URLSessionTaskDelegate {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        completionHandler(nil) // Show 3xx and Location; sending the next request is an explicit action.
    }
}
enum HTTPRequestTool {
    static func send(_ draft: HTTPRequestDraft, configuration: URLSessionConfiguration = .ephemeral) async throws -> HTTPInspectionResult {
        let request = try draft.request()
        configuration.httpShouldSetCookies = false
        configuration.urlCache = nil
        configuration.urlCredentialStorage = nil
        configuration.timeoutIntervalForRequest = 30
        configuration.timeoutIntervalForResource = 60
        let session = URLSession(configuration: configuration, delegate: RequestRedirectPolicy(), delegateQueue: nil)
        defer { session.invalidateAndCancel() }
        let start = ProcessInfo.processInfo.systemUptime
        let (bytes, response) = try await session.bytes(for: request)
        guard let http = response as? HTTPURLResponse else { throw UtilityError("The server did not return an HTTP response.") }
        var data = Data(), truncated = false
        for try await byte in bytes {
            try Task.checkCancellation()
            if data.count >= 2_000_000 { truncated = true; break }
            data.append(byte)
        }
        let duration = ProcessInfo.processInfo.systemUptime - start
        let headers = http.allHeaderFields.map { "\($0.key): \($0.value)" }.sorted().joined(separator: "\n")
        var body = String(data: data, encoding: .utf8) ?? "Binary response (\(data.count.formatted()) bytes)."
        if !truncated, let json = try? DeveloperJSON.parse(body) { body = json.formatted() }
        let status = "HTTP \(http.statusCode) · \(String(format: "%.0f", duration * 1_000)) ms · \(data.count.formatted()) bytes" + (truncated ? " · preview limited to 2 MB" : "")
        return HTTPInspectionResult(status: status, headers: headers, body: body)
    }
}
