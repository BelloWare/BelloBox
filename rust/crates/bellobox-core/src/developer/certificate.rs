//! Offline public X.509 certificate inspection. Parsing is not signature, trust,
//! chain, hostname, or revocation validation. No trust store or network is used.
use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use x509_parser::{
    prelude::{FromDer, X509Certificate},
    public_key::PublicKey,
};
const BEGIN: &str = "-----BEGIN CERTIFICATE-----";
const END: &str = "-----END CERTIFICATE-----";
pub(super) const EXAMPLE: &str = include_str!("../../tests/fixtures/bellobox-example-cert.pem");
fn hex(bytes: &[u8], separator: &str) -> String {
    bytes
        .iter()
        .map(|v| format!("{v:02X}"))
        .collect::<Vec<_>>()
        .join(separator)
}
fn readable(value: impl std::fmt::Display) -> String {
    value
        .to_string()
        .chars()
        .flat_map(|c| {
            if c.is_control() && !matches!(c, '\n' | '\t') {
                c.escape_default().collect::<Vec<_>>()
            } else {
                vec![c]
            }
        })
        .collect()
}
fn iso(seconds: i64) -> Result<String, String> {
    chrono::DateTime::<chrono::Utc>::from_timestamp(seconds, 0)
        .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .ok_or("Certificate validity date is outside the supported range.".into())
}
fn bit_size(bytes: &[u8]) -> Option<usize> {
    let first = bytes.iter().position(|v| *v != 0)?;
    Some((bytes.len() - first - 1) * 8 + (8 - bytes[first].leading_zeros() as usize))
}
fn describe(der: &[u8], index: usize) -> Result<String, String> {
    let (rest, certificate) =
        X509Certificate::from_der(der).map_err(|_| "Invalid X.509 certificate DER.")?;
    if !rest.is_empty() {
        return Err("Certificate DER has trailing bytes; each PEM block must contain exactly one certificate.".into());
    }
    let subject = certificate
        .subject()
        .iter_common_name()
        .find_map(|a| a.as_str().ok())
        .map(readable)
        .unwrap_or_else(|| readable(certificate.subject()));
    let mut lines = vec![
        format!("Certificate {index} · {subject}"),
        format!(
            "Valid from  {}",
            iso(certificate.validity().not_before.timestamp())?
        ),
        format!(
            "Valid until  {}",
            iso(certificate.validity().not_after.timestamp())?
        ),
        format!("SHA-256  {}", hex(&Sha256::digest(der), ":")),
        format!("DER size {} bytes", der.len()),
        format!("Serial   {}", hex(certificate.raw_serial(), "")),
    ];
    let spki = certificate.public_key();
    let oid = spki.algorithm.algorithm.to_id_string();
    let size = match spki.parsed() {
        Ok(PublicKey::RSA(key)) => bit_size(key.modulus),
        Ok(PublicKey::EC(point)) => {
            let curve = spki
                .algorithm
                .parameters
                .as_ref()
                .and_then(|v| v.as_oid().ok())
                .map(|v| v.to_id_string());
            match curve.as_deref() {
                Some("1.2.840.10045.3.1.7") => Some(256),
                Some("1.3.132.0.34") => Some(384),
                Some("1.3.132.0.35") => Some(521),
                _ => {
                    let bits = point.key_size();
                    (bits > 0).then_some(bits)
                }
            }
        }
        Ok(PublicKey::DSA(key)) => bit_size(key),
        _ => match oid.as_str() {
            "1.3.101.112" | "1.3.101.110" => Some(256),
            "1.3.101.113" | "1.3.101.111" => Some(448),
            _ => None,
        },
    };
    if let Some(bits) = size {
        lines.push(format!("Public key size {bits} bits"));
    }
    lines.push(format!("Issuer: {}", readable(certificate.issuer())));
    lines.push(format!("Subject: {}", readable(certificate.subject())));
    lines.push(format!("Version: {}", certificate.version().0 + 1));
    lines.push(format!("Public key algorithm: {oid}"));
    lines.push(format!(
        "Signature algorithm: {}",
        certificate.signature_algorithm.algorithm
    ));
    lines.push(format!(
        "Signature: {}",
        hex(certificate.signature_value.data.as_ref(), "")
    ));
    // Each certificate is at most64KB; extensions have a separate field budget.
    if certificate.extensions().len() > 2_000 {
        return Err("Certificate has more than 2,000 extension fields.".into());
    }
    for ext in certificate.extensions() {
        lines.push(format!(
            "Extension {}{}",
            ext.oid,
            if ext.critical { " (critical)" } else { "" }
        ));
        let detail = format!("{:?}", ext.parsed_extension());
        if detail.len() > 131_072 {
            return Err("Decoded certificate extension exceeds its display limit.".into());
        }
        lines.push(format!("  {}", readable(detail)));
        lines.push(format!("  DER: {}", hex(ext.value, "")));
    }
    Ok(lines.join("\n"))
}
pub(super) fn inspect(input: &str) -> Result<String, String> {
    if input.len() > super::MAX_INPUT {
        return Err("Certificate input exceeds 500,000 UTF-8 bytes.".into());
    }
    if input.contains("PRIVATE KEY") {
        return Err("Paste public PEM certificates only. Private keys are not inspected.".into());
    }
    let mut remaining = input.trim();
    let mut certificates = Vec::new();
    while !remaining.is_empty() {
        if certificates.len() >= 10 || !remaining.starts_with(BEGIN) {
            return Err(
                "Paste up to 10 complete PEM certificate blocks, without extra text.".into(),
            );
        }
        let closing = remaining
            .find(END)
            .ok_or("Close the PEM certificate block.")?;
        let raw = &remaining[BEGIN.len()..closing];
        let compact: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
        if compact.len() > 90_000 {
            return Err("A certificate PEM block exceeds the 64 KB DER limit.".into());
        }
        let der = STANDARD
            .decode(compact)
            .map_err(|_| "Certificate PEM contains invalid Base64.")?;
        if der.is_empty() || der.len() > 65_536 {
            return Err("Each certificate DER must be1–65,536 bytes.".into());
        }
        certificates.push(describe(&der, certificates.len() + 1)?);
        remaining = remaining[closing + END.len()..].trim();
    }
    if certificates.is_empty() {
        return Err("Paste a PEM certificate or load the example.".into());
    }
    let output = certificates.join("\n\n────────\n\n")
        + "\n\nInspection only. Certificate trust, hostname matching, revocation, signatures, and chain validity are not evaluated. No network requests are made.";
    if output.len() > super::MAX_OUTPUT {
        return Err(
            "Certificate details exceed the 4 MB display limit; nothing was truncated.".into(),
        );
    }
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_public_certificate_fields() {
        let result = inspect(EXAMPLE).unwrap();
        for expected in [
            "Bello Box Example",
            "SHA-256",
            "2048 bits",
            "Valid from  2026-09-12T",
            "Valid until  2036-",
            "Issuer:",
            "Subject:",
            "trust, hostname matching",
        ] {
            assert!(result.contains(expected), "{expected}");
        }
    }
    #[test]
    fn multiple_certificates_and_no_trust_claims() {
        let result = inspect(&format!("{EXAMPLE}\n{EXAMPLE}")).unwrap();
        assert!(result.contains("Certificate 2"));
        assert!(result.contains("No network requests"));
        assert!(inspect(&EXAMPLE.repeat(11)).is_err());
    }
    #[test]
    fn rejects_private_incomplete_extra_and_bad_der() {
        for input in [
            "-----BEGIN PRIVATE KEY-----".into(),
            format!("garbage {EXAMPLE}"),
            "-----BEGIN CERTIFICATE-----\nAAAA\n-----END CERTIFICATE-----".into(),
            EXAMPLE.replace(END, ""),
            format!("{EXAMPLE} extra"),
        ] {
            assert!(inspect(&input).is_err());
        }
        let der = STANDARD
            .decode(
                EXAMPLE
                    .replace(BEGIN, "")
                    .replace(END, "")
                    .split_whitespace()
                    .collect::<String>(),
            )
            .unwrap();
        let mut trailing = der;
        trailing.push(0);
        let pem = format!("{BEGIN}\n{}\n{END}", STANDARD.encode(trailing));
        assert!(inspect(&pem).is_err());
    }
    #[test]
    fn exact_modulus_bit_length() {
        assert_eq!(bit_size(&[0, 0x80, 1]), Some(16));
        assert_eq!(bit_size(&[1, 0]), Some(9));
        assert_eq!(bit_size(&[0]), None);
    }
}
