//! IPv4-only source contract from SecurityUtilities.swift. No network operations.
use super::R;
use std::net::Ipv4Addr;

fn source_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{85}' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200b}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
    )
}
fn decimal(text: &str) -> Option<u32> {
    if text.is_empty()
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    text.parse().ok()
}
pub(super) fn run(input: &str) -> R<String> {
    let raw = input.trim_matches(source_whitespace);
    if raw.len() > 64 {
        return Err("Enter one IPv4 CIDR, such as 192.168.1.42/24.".into());
    }
    let parts: Vec<_> = raw.split('/').collect();
    let prefix = (parts.len() == 2)
        .then(|| decimal(parts[1]))
        .flatten()
        .filter(|p| *p <= 32)
        .ok_or("Use IPv4/prefix with a prefix from 0 to 32.")?;
    let octets: Vec<_> = parts[0].split('.').collect();
    if octets.len() != 4 {
        return Err("IPv4 needs four decimal octets.".into());
    }
    let mut address = 0u32;
    for octet in octets {
        let n = decimal(octet)
            .filter(|n| *n <= 255)
            .ok_or("IPv4 octets must be 0–255, with no leading zeros.")?;
        address = (address << 8) | n;
    }
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    let network = address & mask;
    let last = network | !mask;
    let count = 1u64 << (32 - prefix);
    let (first_host, last_host, usable) = if prefix >= 31 {
        (network, last, count)
    } else {
        (network + 1, last - 1, count - 2)
    };
    let broadcast = if prefix >= 31 {
        "Not applicable".into()
    } else {
        Ipv4Addr::from(last).to_string()
    };
    let note = match prefix {
        31 => "Point-to-point /31: both addresses are usable; no directed broadcast.",
        32 => "Single-host /32: one address; no directed broadcast.",
        _ => "Usable range excludes network and broadcast addresses.",
    };
    Ok(format!(
        "Network      {}/{prefix}\nNetmask      {}\nWildcard     {}\nLast address {}\nBroadcast    {broadcast}\nFirst host   {}\nLast host    {}\nAddresses    {count}\nUsable hosts {usable}\n\n{note}",
        Ipv4Addr::from(network),
        Ipv4Addr::from(mask),
        Ipv4Addr::from(!mask),
        Ipv4Addr::from(last),
        Ipv4Addr::from(first_host),
        Ipv4Addr::from(last_host)
    ))
}

#[cfg(test)]
mod tests {
    use super::super::execute;
    fn run(input: &str) -> Result<String, String> {
        execute("subnet", input, "")
    }
    #[test]
    fn subnet_source_exact_example_and_boundary_output() {
        for (input, expected) in [
            (
                "192.168.1.42/24",
                "Network      192.168.1.0/24\nNetmask      255.255.255.0\nWildcard     0.0.0.255\nLast address 192.168.1.255\nBroadcast    192.168.1.255\nFirst host   192.168.1.1\nLast host    192.168.1.254\nAddresses    256\nUsable hosts 254\n\nUsable range excludes network and broadcast addresses.",
            ),
            (
                "255.255.255.255/0",
                "Network      0.0.0.0/0\nNetmask      0.0.0.0\nWildcard     255.255.255.255\nLast address 255.255.255.255\nBroadcast    255.255.255.255\nFirst host   0.0.0.1\nLast host    255.255.255.254\nAddresses    4294967296\nUsable hosts 4294967294\n\nUsable range excludes network and broadcast addresses.",
            ),
            (
                "192.168.1.43/31",
                "Network      192.168.1.42/31\nNetmask      255.255.255.254\nWildcard     0.0.0.1\nLast address 192.168.1.43\nBroadcast    Not applicable\nFirst host   192.168.1.42\nLast host    192.168.1.43\nAddresses    2\nUsable hosts 2\n\nPoint-to-point /31: both addresses are usable; no directed broadcast.",
            ),
            (
                "0.0.0.0/32",
                "Network      0.0.0.0/32\nNetmask      255.255.255.255\nWildcard     0.0.0.0\nLast address 0.0.0.0\nBroadcast    Not applicable\nFirst host   0.0.0.0\nLast host    0.0.0.0\nAddresses    1\nUsable hosts 1\n\nSingle-host /32: one address; no directed broadcast.",
            ),
        ] {
            assert_eq!(run(input).unwrap(), expected, "{input}");
        }
        assert!(
            run("255.255.255.255/32")
                .unwrap()
                .contains("First host   255.255.255.255")
        );
    }
    #[test]
    fn subnet_source_canonical_prefix_and_error_order() {
        let expected = "Use IPv4/prefix with a prefix from 0 to 32.";
        for prefix in [
            "",
            "00",
            "01",
            "+24",
            "-1",
            "33",
            " 24",
            "2 4",
            "２４",
            "24/1",
            "999999999999999999999999",
        ] {
            assert_eq!(
                run(&format!("bad.address/{prefix}")).unwrap_err(),
                expected,
                "{prefix}"
            );
        }
        for input in ["", "1.2.3.4", "/", "1.2.3.4//24"] {
            assert_eq!(run(input).unwrap_err(), expected);
        }
        assert_eq!(run("::1/128").unwrap_err(), expected);
        assert_eq!(
            run("::1/24").unwrap_err(),
            "IPv4 needs four decimal octets."
        );
    }
    #[test]
    fn subnet_source_octet_validation_and_foundation_trimming() {
        for input in ["1.2.3/24", "1.2.3.4.5/24", "/24"] {
            assert_eq!(run(input).unwrap_err(), "IPv4 needs four decimal octets.");
        }
        for octet in [
            "",
            "00",
            "01",
            "256",
            "+1",
            "-1",
            "１",
            "1 ",
            "0x1",
            "99999999999999999999999",
        ] {
            assert_eq!(
                run(&format!("1.2.3.{octet}/24")).unwrap_err(),
                "IPv4 octets must be 0–255, with no leading zeros.",
                "{octet}"
            );
        }
        let normal = run("192.168.1.42/24").unwrap();
        for c in [
            '\t', '\n', '\u{b}', '\u{c}', '\r', ' ', '\u{85}', '\u{a0}', '\u{1680}', '\u{2000}',
            '\u{200b}', '\u{2028}', '\u{2029}', '\u{202f}', '\u{205f}', '\u{3000}',
        ] {
            assert_eq!(run(&format!("{c}192.168.1.42/24{c}")).unwrap(), normal);
        }
        assert!(run("192.168.1.42\n/24").is_err());
    }
    #[test]
    fn subnet_source_trimmed_length_and_wrapper_bounds() {
        assert_eq!(
            run(&"x".repeat(64)).unwrap_err(),
            "Use IPv4/prefix with a prefix from 0 to 32."
        );
        assert_eq!(
            run(&"x".repeat(65)).unwrap_err(),
            "Enter one IPv4 CIDR, such as 192.168.1.42/24."
        );
        assert!(run(&format!("{}0.0.0.0/0{}", " ".repeat(64), " ".repeat(64))).is_ok());
        assert!(run(&" ".repeat(500_001)).unwrap_err().contains("500,000"));
    }
}
