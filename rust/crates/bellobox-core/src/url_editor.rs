//! Bounded full-window URL editing. Lexical spelling is retained independently of
//! WHATWG validation; this is deliberately not a general Foundation URL parser.
use crate::{MAX_INPUT_BYTES, validate_input};

#[derive(Clone, PartialEq, Eq)]
pub struct Parameter {
    pub id: u64,
    pub name: String,
    pub value: String,
    pub has_value: bool,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Draft {
    pub scheme: String,
    pub host: String,
    pub port: String,
    pub path: String,
    pub fragment: String,
    pub parameters: Vec<Parameter>,
    userinfo: String,
    next_id: u64,
}
impl std::fmt::Debug for Draft {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UrlDraft")
            .field("parameter_count", &self.parameters.len())
            .finish_non_exhaustive()
    }
}
impl std::fmt::Debug for Parameter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UrlParameter")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}
fn decode(s: &str) -> Result<String, String> {
    let b = s.as_bytes();
    for i in 0..b.len() {
        if b[i] == b'%'
            && (i + 2 >= b.len() || !b[i + 1].is_ascii_hexdigit() || !b[i + 2].is_ascii_hexdigit())
        {
            return Err("Invalid percent escape.".into());
        }
    }
    percent_encoding::percent_decode_str(s)
        .decode_utf8()
        .map(|s| s.into_owned())
        .map_err(|_| "Percent-decoded text is not UTF-8.".into())
}
fn encode(s: &str, path: bool, fragment: bool) -> String {
    let mut out = String::with_capacity(s.len());
    const HEX: &[u8] = b"0123456789ABCDEF";
    for b in s.bytes() {
        let allowed = b.is_ascii_alphanumeric()
            || b"-._~!$'()*+,;:@/".contains(&b)
            || (path && b == b'&')
            || ((fragment || !path) && b == b'?')
            || (fragment && b"&=".contains(&b))
            || (path && b == b'=');
        if allowed {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 15) as usize] as char);
        }
    }
    out
}
impl Draft {
    pub fn inspect(input: &str) -> Result<Self, String> {
        validate_input(input)?;
        let s = input.trim();
        if s.chars().any(|c| c.is_control() || c == '\\') {
            return Err("Control characters and backslashes are not supported in URLs.".into());
        }
        let (scheme, rest) = s
            .split_once("://")
            .ok_or("Enter a complete http:// or https:// URL.")?;
        if !matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") {
            return Err("Enter a complete http:// or https:// URL.".into());
        }
        url::Url::parse(s).map_err(|_| "The URL is invalid.")?;
        let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
        let authority = &rest[..end];
        let (userinfo, authority) = if let Some((u, a)) = authority.rsplit_once('@') {
            if u.contains('@') || u.chars().any(char::is_whitespace) {
                return Err("Ambiguous URL authority is not supported.".into());
            }
            decode(u)?;
            (format!("{u}@"), a)
        } else {
            (String::new(), authority)
        };
        if authority.ends_with(':') {
            return Err("An explicit port cannot be empty.".into());
        }
        let (host, port) = if authority.starts_with('[') {
            let end = authority.find(']').ok_or("Invalid IPv6 host.")?;
            let suffix = &authority[end + 1..];
            (
                &authority[..=end],
                if suffix.is_empty() {
                    ""
                } else {
                    suffix.strip_prefix(':').ok_or("Invalid URL authority.")?
                },
            )
        } else if let Some((h, p)) = authority.rsplit_once(':') {
            (h, p)
        } else {
            (authority, "")
        };
        let tail = &rest[end..];
        let (tail, fragment) = tail.split_once('#').unwrap_or((tail, ""));
        let (path, query) = tail
            .split_once('?')
            .map(|(a, b)| (a, Some(b)))
            .unwrap_or((tail, None));
        let parameters = query
            .filter(|q| !q.is_empty())
            .map(|q| {
                q.split('&')
                    .enumerate()
                    .map(|(i, p)| {
                        let (n, v) = p
                            .split_once('=')
                            .map(|(n, v)| (n, Some(v)))
                            .unwrap_or((p, None));
                        Ok(Parameter {
                            id: i as u64,
                            name: decode(n)?,
                            value: decode(v.unwrap_or(""))?,
                            has_value: v.is_some(),
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()
            })
            .transpose()?
            .unwrap_or_default();
        let draft = Self {
            scheme: scheme.to_owned(),
            host: host.to_owned(),
            port: port.to_owned(),
            path: decode(path)?,
            fragment: decode(fragment)?,
            next_id: parameters.len() as u64,
            parameters,
            userinfo,
        };
        draft.validate()?;
        Ok(draft)
    }
    pub fn validate(&self) -> Result<(), String> {
        let mut ids = std::collections::HashSet::new();
        if self.parameters.iter().any(|p| !ids.insert(p.id)) {
            return Err("Duplicate parameter identities are invalid.".into());
        }
        let size = [
            &self.scheme,
            &self.host,
            &self.port,
            &self.path,
            &self.fragment,
            &self.userinfo,
        ]
        .iter()
        .map(|s| s.len())
        .sum::<usize>()
            + self
                .parameters
                .iter()
                .map(|p| p.name.len() + p.value.len() + 1 + usize::from(p.has_value))
                .sum::<usize>();
        if size > MAX_INPUT_BYTES {
            return Err("Edited URL fields exceed 500000 UTF-8 bytes; no result was built.".into());
        }
        if !matches!(self.scheme.to_ascii_lowercase().as_str(), "http" | "https") {
            return Err("Choose http or https.".into());
        }
        if self.host.is_empty()
            || self
                .host
                .chars()
                .any(|c| c.is_whitespace() || c.is_control() || "/?#@%\\".contains(c))
        {
            return Err("The edited host is invalid or unsupported.".into());
        }
        if (!self.host.starts_with('[') && self.host.contains(':'))
            || (self.host.starts_with('[') && !self.host.ends_with(']'))
        {
            return Err("IPv6 hosts must use brackets.".into());
        }
        if !self.port.is_empty()
            && (!self.port.bytes().all(|b| b.is_ascii_digit())
                || self.port.parse::<u16>().ok().filter(|p| *p > 0).is_none())
        {
            return Err("The port must be between 1 and 65535.".into());
        }
        if !self.path.is_empty() && !self.path.starts_with('/') {
            return Err("The path must be empty or begin with /.".into());
        }
        Ok(())
    }
    pub fn add(&mut self) -> Result<u64, String> {
        let id = self
            .parameters
            .iter()
            .map(|p| p.id)
            .max()
            .map(|id| id.checked_add(1).ok_or("Too many parameter edits."))
            .transpose()?
            .unwrap_or(0)
            .max(self.next_id);
        self.next_id = id.checked_add(1).ok_or("Too many parameter edits.")?;
        self.parameters.push(Parameter {
            id,
            name: String::new(),
            value: String::new(),
            has_value: true,
        });
        if let Err(e) = self.validate() {
            self.parameters.pop();
            return Err(e);
        }
        Ok(id)
    }
    pub fn remove(&mut self, id: u64) {
        self.parameters.retain(|p| p.id != id);
    }
    pub fn build(&self) -> Result<String, String> {
        self.validate()?;
        let mut out = format!("{}://{}{}", self.scheme, self.userinfo, self.host);
        if !self.port.is_empty() {
            out.push(':');
            out.push_str(&self.port);
        }
        out.push_str(&encode(&self.path, true, false));
        if !self.parameters.is_empty() {
            out.push('?');
        }
        for (i, p) in self.parameters.iter().enumerate() {
            if i > 0 {
                out.push('&');
            }
            out.push_str(&encode(&p.name, false, false));
            if p.has_value {
                out.push('=');
                out.push_str(&encode(&p.value, false, false));
            }
        }
        if !self.fragment.is_empty() {
            out.push('#');
            out.push_str(&encode(&self.fragment, false, true));
        }
        if out.len() > 4_000_000 {
            return Err("Rebuilt URL exceeds 4000000 UTF-8 bytes.".into());
        }
        let u = url::Url::parse(&out).map_err(|_| "The edited URL is invalid.")?;
        if u.host_str().is_none() {
            return Err("The edited host is invalid.".into());
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordered_flags_blank_plus_and_components() {
        let mut d = Draft::inspect(
            "https://Example.COM:443/a%20b?q=a+b&tag=one&tag=two&flag&empty=#hi%20there",
        )
        .unwrap();
        assert_eq!(d.parameters.len(), 5);
        assert!(!d.parameters[3].has_value);
        assert!(d.parameters[4].has_value);
        assert_eq!(d.parameters[0].value, "a+b");
        assert_eq!(d.path, "/a b");
        assert_eq!(
            d.build().unwrap(),
            "https://Example.COM:443/a%20b?q=a+b&tag=one&tag=two&flag&empty=#hi%20there"
        );
        d.parameters[0].value = "a&b=c% #".into();
        assert!(d.build().unwrap().contains("q=a%26b%3Dc%25%20%23"));
    }
    #[test]
    fn lexical_authority_and_stable_ids() {
        let mut d = Draft::inspect("http://synthetic:example@Example.COM:80/a/../b").unwrap();
        assert_eq!(
            d.build().unwrap(),
            "http://synthetic:example@Example.COM:80/a/../b"
        );
        let a = d.add().unwrap();
        let b = d.add().unwrap();
        d.remove(a);
        let c = d.add().unwrap();
        assert!(c > b);
        assert_eq!(d.parameters[0].id, b);
        assert_eq!(
            Draft::inspect("https://example.com")
                .unwrap()
                .build()
                .unwrap(),
            "https://example.com"
        );
    }
    #[test]
    fn complete_large_list_last_edit() {
        let s = format!(
            "https://example.com/?{}",
            (0..3001)
                .map(|i| format!("x={i}"))
                .collect::<Vec<_>>()
                .join("&")
        );
        let mut d = Draft::inspect(&s).unwrap();
        d.parameters[3000].value = "last +".into();
        let result = d.build().unwrap();
        assert_eq!(d.parameters.len(), 3001);
        assert!(result.ends_with("&x=last%20+"));
        assert_eq!(Draft::inspect(&result).unwrap().parameters.len(), 3001);
    }
    #[test]
    fn malformed_and_bounds_fail_closed() {
        for s in [
            "file:///tmp/a",
            "https://x/%GG",
            "https://x/%FF",
            "https://x\\evil/a",
            "https://a@b@x/",
            "https://x:0/",
        ] {
            assert!(Draft::inspect(s).is_err(), "{s}");
        }
        let mut d = Draft::inspect("https://example.com/").unwrap();
        d.path = "x".repeat(MAX_INPUT_BYTES);
        assert!(d.build().is_err());
        d.path = "/".into();
        d.host = "evil/x".into();
        assert!(d.build().is_err());
        d.host = "example.com".into();
        d.port = "65536".into();
        assert!(d.build().is_err());
    }
    #[test]
    fn authority_rejection_and_identity_bounds() {
        for source in [
            "https://x:/",
            "https://[invalid]/",
            "https://x/a\nb",
            "https://[::1]evil/",
        ] {
            assert!(Draft::inspect(source).is_err());
        }
        let mut d = Draft::inspect("https://[::1]:8443/?x=1&x=2").unwrap();
        assert_eq!(d.host, "[::1]");
        assert_eq!(d.build().unwrap(), "https://[::1]:8443/?x=1&x=2");
        d.parameters[1].id = d.parameters[0].id;
        assert!(d.build().is_err());
        d.parameters[1].id = 100;
        assert_eq!(d.add().unwrap(), 101);
        d.remove(101);
        assert_eq!(d.add().unwrap(), 102);
        d.parameters[0].value = "a".repeat(250001);
        d.parameters[1].value = "b".repeat(250001);
        assert!(d.build().is_err());
        assert!(!format!("{d:?}").contains("250001"));
    }
}
