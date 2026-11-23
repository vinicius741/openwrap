//! Credentials used for one connection plan.
//!
//! PIN + TOTP profiles keep the *secret*, not a generated password, so every
//! attempt (first connect and each retry) sends a fresh one-time password.
//! Reusing the password from an earlier attempt fails once its 30-second step
//! has passed, and servers that reject replayed codes refuse it even sooner.

use chrono::{DateTime, Utc};

use crate::errors::AppError;
use crate::secrets::{totp::generate_totp, StoredSecret, StoredSecretKind};

/// Length of one TOTP time step (RFC 6238 default, also used by `generate_totp`).
pub const TOTP_STEP_SECONDS: i64 = 30;

#[derive(Clone)]
pub enum PlanCredentials {
    /// The profile does not use `auth-user-pass`.
    None,
    /// Username and password typed into the credential prompt.
    Static { username: String, password: String },
    /// PIN + TOTP: the password is generated again for every attempt.
    Generated(StoredSecret),
}

impl PlanCredentials {
    pub fn is_generated(&self) -> bool {
        matches!(self, Self::Generated(_))
    }
}

/// Username and password for one attempt, plus the TOTP step used (if any).
pub struct AttemptCredentials {
    pub username: Option<String>,
    pub password: Option<String>,
    pub otp_step: Option<u64>,
}

pub fn resolve_attempt_credentials(
    credentials: &PlanCredentials,
    now: DateTime<Utc>,
) -> Result<AttemptCredentials, AppError> {
    match credentials {
        PlanCredentials::None => Ok(AttemptCredentials {
            username: None,
            password: None,
            otp_step: None,
        }),
        PlanCredentials::Static { username, password } => Ok(AttemptCredentials {
            username: Some(username.clone()),
            password: Some(password.clone()),
            otp_step: None,
        }),
        PlanCredentials::Generated(secret) => {
            let (username, password) = generate_password(secret, now)?;
            Ok(AttemptCredentials {
                username: Some(username),
                password: Some(password),
                otp_step: Some(totp_step(now)),
            })
        }
    }
}

/// `pin + current TOTP code` for a generated-password profile.
pub fn generate_password(
    secret: &StoredSecret,
    now: DateTime<Utc>,
) -> Result<(String, String), AppError> {
    if secret.kind != StoredSecretKind::PinTotp {
        return Err(AppError::Settings(
            "This profile expects a generated VPN password, but the saved local credentials are not in the generated-password format.".into(),
        ));
    }

    let pin = secret.pin.as_deref().ok_or_else(|| {
        AppError::Settings("The generated-password profile is missing its saved PIN.".into())
    })?;
    if !is_valid_pin(pin) {
        return Err(AppError::Settings(
            "The saved VPN PIN must contain exactly 4 digits.".into(),
        ));
    }

    let totp_secret = secret.totp_secret.as_deref().ok_or_else(|| {
        AppError::Settings(
            "The generated-password profile is missing its saved TOTP secret.".into(),
        )
    })?;
    let otp = generate_totp(totp_secret, now)?;
    Ok((secret.username.clone(), format!("{pin}{otp}")))
}

/// PINs for generated passwords are exactly four ASCII digits.
pub fn is_valid_pin(pin: &str) -> bool {
    pin.len() == 4 && pin.chars().all(|ch| ch.is_ascii_digit())
}

pub fn totp_step(at: DateTime<Utc>) -> u64 {
    (at.timestamp() / TOTP_STEP_SECONDS) as u64
}

/// Retry delay that also waits until the TOTP step after `last_step`, so the
/// retry never sends the code the server has already seen.
pub fn retry_delay_for_fresh_otp(
    base_delay_seconds: u64,
    last_step: Option<u64>,
    now: DateTime<Utc>,
) -> u64 {
    let Some(step) = last_step else {
        return base_delay_seconds;
    };
    let next_step_starts_at = (step as i64 + 1) * TOTP_STEP_SECONDS;
    // One extra second absorbs clock rounding between here and the attempt.
    let wait = (next_step_starts_at - now.timestamp()).max(0) as u64 + 1;
    base_delay_seconds.max(wait)
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::profiles::ProfileId;

    fn at(seconds: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(seconds, 0).unwrap()
    }

    #[test]
    fn retry_waits_for_the_next_totp_step() {
        // Code from step 100 (t = 3000..3029). At t = 3005 the next step starts
        // in 25 s, so a 2 s backoff becomes 26 s.
        assert_eq!(retry_delay_for_fresh_otp(2, Some(100), at(3005)), 26);
        // Late in the step the backoff already crosses into the next step.
        assert_eq!(retry_delay_for_fresh_otp(10, Some(100), at(3028)), 10);
        // The step already passed: keep the backoff.
        assert_eq!(retry_delay_for_fresh_otp(5, Some(100), at(3100)), 5);
        // No OTP involved.
        assert_eq!(retry_delay_for_fresh_otp(2, None, at(3005)), 2);
    }

    #[test]
    fn generated_credentials_use_a_fresh_code_per_step() {
        let secret = StoredSecret::pin_totp(
            ProfileId::new(),
            "alice".into(),
            "1234".into(),
            "JBSWY3DPEHPK3PXP".into(),
        );
        let credentials = PlanCredentials::Generated(secret);

        let first = resolve_attempt_credentials(&credentials, at(3005)).unwrap();
        let same_step = resolve_attempt_credentials(&credentials, at(3020)).unwrap();
        let next_step = resolve_attempt_credentials(&credentials, at(3031)).unwrap();

        assert_eq!(first.username.as_deref(), Some("alice"));
        assert!(first.password.as_deref().unwrap().starts_with("1234"));
        assert_eq!(first.otp_step, Some(100));
        assert_eq!(first.password, same_step.password);
        assert_ne!(first.password, next_step.password);
        assert_eq!(next_step.otp_step, Some(101));
    }

    #[test]
    fn rejects_invalid_pins() {
        assert!(is_valid_pin("0042"));
        assert!(!is_valid_pin("42"));
        assert!(!is_valid_pin("12a4"));
        assert!(!is_valid_pin("12345"));
    }
}
