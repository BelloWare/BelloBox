//! Bounded SQL layout lexer, ported from SQLFormatterTool.swift. It never executes
//! or claims to validate SQL. Literal/comment tokens are preserved byte-for-byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Word,
    Number,
    Literal,
    Symbol,
    LineComment,
    BlockComment,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Token {
    text: String,
    kind: Kind,
    newline_before: bool,
}
const KEYWORDS: &[&str] = &[
    "SELECT",
    "DISTINCT",
    "FROM",
    "WHERE",
    "AND",
    "OR",
    "NOT",
    "NULL",
    "IS",
    "IN",
    "AS",
    "ON",
    "JOIN",
    "LEFT",
    "RIGHT",
    "FULL",
    "INNER",
    "OUTER",
    "CROSS",
    "GROUP",
    "ORDER",
    "BY",
    "ASC",
    "DESC",
    "HAVING",
    "LIMIT",
    "OFFSET",
    "INSERT",
    "INTO",
    "VALUES",
    "UPDATE",
    "SET",
    "DELETE",
    "RETURNING",
    "WITH",
    "UNION",
    "ALL",
    "EXCEPT",
    "INTERSECT",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "TRUE",
    "FALSE",
    "BETWEEN",
    "LIKE",
    "EXISTS",
    "CREATE",
    "TABLE",
    "DROP",
    "ALTER",
];
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}
fn is_newline(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{0085}' | '\u{2028}' | '\u{2029}')
}
fn tokenize(text: &str, dialect: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = text.chars().take(150_001).collect();
    let n = chars.len();
    if n > 150_000 {
        return Err("Format up to 150,000 SQL characters at a time.".into());
    }
    let mut i = 0;
    let mut tokens = Vec::new();
    let mut newline = false;
    while i < n {
        if chars[i].is_whitespace() {
            newline |= is_newline(chars[i]);
            i += 1;
            continue;
        }
        let start = i;
        let kind;
        let at = |offset: usize| chars.get(i + offset).copied();
        if chars[i] == '-'
            && at(1) == Some('-')
            && (dialect != "MySQL" || at(2).is_none_or(char::is_whitespace))
            || dialect == "MySQL" && chars[i] == '#'
        {
            kind = Kind::LineComment;
            i += if chars[i] == '#' { 1 } else { 2 };
            while i < n && !is_newline(chars[i]) {
                i += 1;
            }
        } else if chars[i] == '/' && at(1) == Some('*') {
            kind = Kind::BlockComment;
            i += 2;
            let mut depth = 1;
            while i < n && depth > 0 {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
                if depth > 64 {
                    return Err("SQL comment nesting exceeds 64 levels.".into());
                }
            }
            if depth != 0 {
                return Err("Close the /* block comment before formatting.".into());
            }
        } else if chars[i] == '$' && dollar_end(&chars, i).is_some() {
            kind = Kind::Literal;
            let closing = dollar_end(&chars, i).expect("checked dollar delimiter");
            let delimiter = &chars[i..=closing];
            i = closing + 1;
            let mut found = false;
            while i + delimiter.len() <= n {
                if &chars[i..i + delimiter.len()] == delimiter {
                    i += delimiter.len();
                    found = true;
                    break;
                }
                i += 1;
            }
            if !found {
                return Err("Close the SQL dollar-quoted string.".into());
            }
        } else if ['\'', '"', '`', '['].contains(&chars[i])
            || ['e', 'E', 'b', 'B', 'x', 'X', 'n', 'N'].contains(&chars[i]) && at(1) == Some('\'')
            || ['u', 'U'].contains(&chars[i])
                && at(1) == Some('&')
                && matches!(at(2), Some('\'' | '"'))
        {
            let escaped = chars[i] == 'e' || chars[i] == 'E' || dialect == "MySQL";
            if ['u', 'U'].contains(&chars[i]) && chars.get(i + 1) == Some(&'&') {
                i += 2;
            }
            if chars.get(i + 1) == Some(&'\'') && !['\'', '"', '`', '['].contains(&chars[i]) {
                i += 1;
            }
            let quote = if chars[i] == '[' { ']' } else { chars[i] };
            kind = Kind::Literal;
            i += 1;
            let mut found = false;
            while i < n {
                if escaped && chars[i] == '\\' {
                    i = (i + 2).min(n);
                    continue;
                }
                if chars[i] == quote {
                    i += 1;
                    if chars.get(i) == Some(&quote) {
                        i += 1;
                        continue;
                    }
                    found = true;
                    break;
                }
                i += 1;
            }
            if !found {
                return Err("Close the quoted SQL literal or identifier.".into());
            }
        } else if dialect == "MySQL" && chars[i] == '-' && at(1) == Some('-') {
            kind = Kind::Symbol;
            i += 1;
        } else if [':', '@', '?'].contains(&chars[i])
            && at(1)
                .is_some_and(|c| c.is_alphabetic() || c == '_' || chars[i] == '?' && c.is_numeric())
        {
            kind = Kind::Literal;
            i += 1;
            while i < n && is_word(chars[i]) {
                i += 1;
            }
        } else if chars[i].is_numeric() || chars[i] == '.' && at(1).is_some_and(char::is_numeric) {
            kind = Kind::Number;
            if chars[i] == '0' && matches!(at(1), Some('x' | 'X')) {
                i += 2;
                while i < n && chars[i].is_ascii_hexdigit() {
                    i += 1;
                }
            } else {
                while i < n && chars[i].is_numeric() {
                    i += 1;
                }
                if chars.get(i) == Some(&'.') {
                    i += 1;
                    while i < n && chars[i].is_numeric() {
                        i += 1;
                    }
                }
                if chars.get(i).is_some_and(|c| *c == 'e' || *c == 'E') {
                    i += 1;
                    if chars.get(i).is_some_and(|c| *c == '+' || *c == '-') {
                        i += 1;
                    }
                    while i < n && chars[i].is_numeric() {
                        i += 1;
                    }
                }
            }
        } else if is_word(chars[i]) {
            kind = Kind::Word;
            i += 1;
            while i < n && is_word(chars[i]) {
                i += 1;
            }
        } else {
            kind = Kind::Symbol;
            i += 1;
            if "=<>!|&+*/%~^-:?@#".contains(chars[start]) {
                while i < n && "=<>!|&+*/%~^-:?@#".contains(chars[i]) {
                    if chars[i] == '-' && chars.get(i + 1) == Some(&'-')
                        || chars[i] == '/' && chars.get(i + 1) == Some(&'*')
                    {
                        break;
                    }
                    i += 1;
                }
            }
        }
        tokens.push(Token {
            text: chars[start..i].iter().collect(),
            kind,
            newline_before: newline,
        });
        newline = false;
        if tokens.len() > 30_000 {
            return Err("SQL exceeds the 30,000-token formatting limit.".into());
        }
    }
    Ok(tokens)
}
fn dollar_end(chars: &[char], start: usize) -> Option<usize> {
    let closing = (start + 1..chars.len().min(start + 65)).find(|&i| chars[i] == '$')?;
    if closing == start + 1
        || (chars[start + 1].is_alphabetic() || chars[start + 1] == '_')
            && chars[start + 1..closing]
                .iter()
                .all(|c| c.is_alphanumeric() || *c == '_')
    {
        Some(closing)
    } else {
        None
    }
}
fn append(line: &mut String, text: &str, tight: bool, depth: usize) {
    if line.is_empty() {
        line.push_str(&"  ".repeat(depth.min(20)));
        line.push_str(text);
    } else {
        if !tight {
            line.push(' ');
        }
        line.push_str(text);
    }
}
fn flush(line: &mut String, lines: &mut Vec<String>) {
    if !line.trim().is_empty() {
        lines.push(std::mem::take(line));
    }
}
fn format(input: &str, uppercase: bool, dialect: &str) -> Result<String, String> {
    let tokens = tokenize(input, dialect)?;
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut depth = 0_usize;
    let starters = [
        "SELECT",
        "FROM",
        "WHERE",
        "HAVING",
        "LIMIT",
        "OFFSET",
        "VALUES",
        "SET",
        "RETURNING",
        "UNION",
        "EXCEPT",
        "INTERSECT",
        "WITH",
    ];
    for (i, token) in tokens.iter().enumerate() {
        let previous = i.checked_sub(1).map(|i| &tokens[i]);
        let upper = token.text.to_uppercase();
        let word = token.kind == Kind::Word;
        let text = if word && uppercase && KEYWORDS.contains(&upper.as_str()) {
            upper.as_str()
        } else {
            token.text.as_str()
        };
        let next = tokens
            .get(i + 1)
            .map(|t| t.text.to_uppercase())
            .unwrap_or_default();
        if token.kind == Kind::LineComment {
            append(&mut line, text, false, depth);
            flush(&mut line, &mut lines);
            continue;
        }
        if token.kind == Kind::BlockComment {
            flush(&mut line, &mut lines);
            append(&mut line, text, false, depth);
            flush(&mut line, &mut lines);
            continue;
        }
        if word
            && (starters.contains(&upper.as_str())
                || ["GROUP", "ORDER"].contains(&upper.as_str()) && next == "BY"
                || ["LEFT", "RIGHT", "FULL", "INNER", "CROSS"].contains(&upper.as_str())
                    && ["JOIN", "OUTER"].contains(&next.as_str())
                || upper == "JOIN"
                    && !previous.is_some_and(|p| {
                        ["LEFT", "RIGHT", "FULL", "INNER", "CROSS", "OUTER"]
                            .contains(&p.text.to_uppercase().as_str())
                    }))
        {
            flush(&mut line, &mut lines);
        }
        if token.kind == Kind::Literal
            && previous.is_some_and(|p| p.kind == Kind::Literal)
            && token.newline_before
        {
            flush(&mut line, &mut lines);
        }
        if text == ")" {
            depth = depth.saturating_sub(1);
        }
        let tight = [",", ";", ")", ".", "::"].contains(&text)
            || previous.is_some_and(|p| ["(", ".", "::"].contains(&p.text.as_str()))
            || text == "("
                && previous.is_some_and(|p| {
                    p.kind == Kind::Word && !KEYWORDS.contains(&p.text.to_uppercase().as_str())
                });
        append(&mut line, text, tight, depth);
        if text == "(" {
            depth += 1;
            if depth > 64 {
                return Err("SQL parentheses exceed 64 levels.".into());
            }
        }
        if text == ";" || text == "," && depth == 0 {
            flush(&mut line, &mut lines);
        }
    }
    flush(&mut line, &mut lines);
    Ok(lines.join("\n"))
}
pub(super) fn run(input: &str, option: &str) -> Result<String, String> {
    let (mut dialect, mut uppercase) = ("PostgreSQL".to_string(), true);
    if option.trim().starts_with('{') {
        let value = super::parse_json(option)?;
        let map = value.as_object().ok_or("SQL options must be an object.")?;
        if map
            .keys()
            .any(|key| !matches!(key.as_str(), "dialect" | "keywords"))
        {
            return Err("SQL options support dialect and keywords only.".into());
        }
        if let Some(value) = map.get("dialect") {
            dialect = value
                .as_str()
                .ok_or("SQL dialect must be a string.")?
                .into();
        }
        if let Some(value) = map.get("keywords") {
            uppercase = match value.as_str() {
                Some("Uppercase") => true,
                Some("Preserve") => false,
                _ => return Err("SQL keywords must be Uppercase or Preserve.".into()),
            };
        }
    } else if !option.trim().is_empty() {
        dialect = option.trim().into();
    }
    if !["PostgreSQL", "SQLite", "MySQL"].contains(&dialect.as_str()) {
        return Err("SQL dialect must be PostgreSQL, SQLite, or MySQL.".into());
    }
    format(input, uppercase, &dialect)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn normalized(s: &str, dialect: &str) -> Vec<String> {
        tokenize(s, dialect)
            .unwrap()
            .iter()
            .map(|t| {
                if t.kind == Kind::Word && KEYWORDS.contains(&t.text.to_uppercase().as_str()) {
                    t.text.to_uppercase()
                } else {
                    t.text.clone()
                }
            })
            .collect()
    }
    #[test]
    fn source_literals_and_tokens_are_preserved() {
        let input=r#"select .5,1e-3,0xFF,E'a\'b',"Case",[a]]b],:name,@param,$1,?12, $$line; -- raw$$, foo::int from t where a<=2 and b->>'name'='x''y';"#.to_owned()+"\n-- trailing comment\nselect 'a'\n'b'; /* nested /* inner */ outer */";
        let output = run(&input, "").unwrap();
        assert_eq!(
            normalized(&input, "PostgreSQL"),
            normalized(&output, "PostgreSQL")
        );
        assert!(output.contains("'a'\n'b'"));
        assert_eq!(run(&output, "").unwrap(), output);
    }
    #[test]
    fn dialect_comments_escape_and_operators_keep_meaning() {
        for (sql, dialect) in [
            ("select 1+-- inline\n2;", "PostgreSQL"),
            (r#"select U&'d\0061t', U&"d\0061t";"#, "PostgreSQL"),
            ("select 'a\\'b', a--b;\n# comment\nselect 1;", "MySQL"),
        ] {
            let options = serde_json::json!({"dialect":dialect,"keywords":"Preserve"}).to_string();
            let output = run(sql, &options).unwrap();
            assert_eq!(
                tokenize(sql, dialect)
                    .unwrap()
                    .iter()
                    .map(|t| &t.text)
                    .collect::<Vec<_>>(),
                tokenize(&output, dialect)
                    .unwrap()
                    .iter()
                    .map(|t| &t.text)
                    .collect::<Vec<_>>()
            );
        }
    }
    #[test]
    fn malformed_and_bounded_input_rejected() {
        for s in [
            "select 'unclosed",
            "select /* unclosed",
            "select $tag$unclosed",
        ] {
            assert!(run(s, "").is_err());
        }
        assert!(run(&"(".repeat(65), "").is_err());
        assert!(run(&"x ".repeat(30_001), "").is_err());
        assert!(run(&"x".repeat(150_001), "").is_err());
        assert!(run("select 1", r#"{"dialect":"Oracle"}"#).is_err());
    }
    #[test]
    fn unicode_and_idempotent_clauses() {
        let input = "select 名称, count(*) from café left join x on x.id=café.id where x.note='A;B' order by 名称;";
        let result = run(input, "").unwrap();
        assert_eq!(
            normalized(input, "PostgreSQL"),
            normalized(&result, "PostgreSQL")
        );
        assert_eq!(result, run(&result, "").unwrap());
        assert!(result.contains("LEFT JOIN"));
        assert!(result.contains("ORDER BY"));
    }
}
