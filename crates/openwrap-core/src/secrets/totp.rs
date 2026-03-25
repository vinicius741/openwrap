use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use sha1::Sha1;

use crate::errors::AppError;

type HmacSha1 = Hmac<Sha1>;

pub fn generate_totp(secret_base32: &str, at: DateTime<Utc>) -> Result<String, AppError> {
    let secret = decode_base32(secret_base32)?;
    let counter = (at.timestamp() / 30) as u64;
    let mut mac =
        HmacSha1::new_from_slice(&secret).map_err(|error| AppError::Settings(error.to_string()))?;
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 0x0f) as usize;
    let binary = ((digest[offset] as u32 & 0x7f) << 24)
        | ((digest[offset + 1] as u32) << 16)
        | ((digest[offset + 2] as u32) << 8)
        | (digest[offset + 3] as u32);
    let code = binary % 1_000_000;
    Ok(format!("{code:06}"))
}

fn decode_base32(input: &str) -> Result<Vec<u8>, AppError> {
    let mut output = Vec::new();
    let mut buffer = 0u32;
    let mut bits_left = 0u8;

    for ch in input.chars() {
        if matches!(ch, ' ' | '\t' | '\n' | '\r' | '-') {
            continue;
        }
        if ch == '=' {
            break;
        }

        let upper = ch.to_ascii_uppercase();
        let value = match upper {
            'A'..='Z' => upper as u8 - b'A',
            '2'..='7' => 26 + (upper as u8 - b'2'),
            _ => {
                return Err(AppError::Settings(
                    "TOTP secret must be a valid base32 string.".into(),
                ))
            }
        } as u32;

        buffer = (buffer << 5) | value;
        bits_left += 5;
        while bits_left >= 8 {
            bits_left -= 8;
            output.push(((buffer >> bits_left) & 0xff) as u8);
        }
    }

    if output.is_empty() {
        return Err(AppError::Settings(
            "TOTP secret must be a valid base32 string.".into(),
        ));
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::generate_totp;

    #[test]
    fn generates_known_rfc_totp_vector() {
        let secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
        let code = generate_totp(secret, chrono::Utc.timestamp_opt(59, 0).unwrap()).unwrap();
        assert_eq!(code, "287082");
    }

    #[test]
    fn rejects_invalid_base32() {
        let error = generate_totp("not-valid-!", chrono::Utc::now()).unwrap_err();
        assert!(error.to_string().contains("valid base32"));
    }

    #[test]
    fn accepts_lowercase_base32() {
        let code = generate_totp(
            "gezdgnbvgy3tqojqgezdgnbvgy3tqojq",
            chrono::Utc.timestamp_opt(59, 0).unwrap(),
        )
        .unwrap();
        assert_eq!(code, "287082");
    }
}
