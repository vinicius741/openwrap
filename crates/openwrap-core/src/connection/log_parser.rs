use chrono::Utc;

use crate::connection::session::{LogEntry, LogLevel};
use crate::errors::UserFacingError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedLogSignal {
    Connected,
    RetryableFailure,
    AuthFailed,
    DnsHint,
    None,
}

pub fn sanitize_log(stream: &str, line: &str) -> LogEntry {
    let mut message = line.to_string();
    let mut sanitized = false;
    for marker in ["PASSWORD", "password", "AUTH_FAILED,CRV1"] {
        if message.contains(marker) {
            message = message.replace(marker, "[redacted]");
            sanitized = true;
        }
    }

    let (level, classification) = if line.contains("AUTH_FAILED") {
        (LogLevel::Error, "auth_failed")
    } else if line.contains("Initialization Sequence Completed") {
        (LogLevel::Info, "connected")
    } else if line.contains("SIGUSR1") || line.contains("Restart pause") {
        (LogLevel::Warn, "retryable")
    } else if line.contains("dhcp-option")
        || line.contains("PUSH_REPLY")
        || line.contains("OPENWRAP_DNS_WARNING:")
    {
        (LogLevel::Info, "dns")
    } else if let Some((level, classification)) = error_classification(line) {
        (level, classification)
    } else {
        (LogLevel::Debug, "default")
    };

    LogEntry {
        ts: Utc::now(),
        stream: stream.to_string(),
        level,
        message,
        sanitized,
        classification: classification.to_string(),
    }
}

pub fn classify_signal(line: &str) -> ParsedLogSignal {
    if line.contains("Initialization Sequence Completed") {
        ParsedLogSignal::Connected
    } else if line.contains("AUTH_FAILED") {
        ParsedLogSignal::AuthFailed
    } else if line.contains("SIGUSR1") || line.contains("Restart pause") {
        ParsedLogSignal::RetryableFailure
    } else if line.contains("dhcp-option")
        || line.contains("PUSH_REPLY")
        || line.contains("OPENWRAP_DNS_WARNING:")
    {
        ParsedLogSignal::DnsHint
    } else {
        ParsedLogSignal::None
    }
}

pub fn diagnose_exit_error<'a>(
    code: Option<i32>,
    logs: impl DoubleEndedIterator<Item = &'a LogEntry>,
) -> Option<UserFacingError> {
    let detail = logs
        .rev()
        .find_map(|entry| relevant_failure_detail(entry))?;
    let details_safe = Some(detail.clone());

    if contains_any(
        &detail,
        &[
            "cannot resolve host address",
            "temporary failure in name resolution",
        ],
    ) {
        Some(UserFacingError {
            code: "openvpn_host_resolution_failed".into(),
            title: "Server hostname could not be resolved".into(),
            message: exit_message(
                code,
                "OpenVPN could not resolve the VPN server hostname during startup.",
            ),
            suggested_fix: Some(
                "Check the profile's server address, local DNS resolution, and network connectivity."
                    .into(),
            ),
            details_safe,
        })
    } else if contains_any(
        &detail,
        &["options error", "unrecognized option", "unknown option"],
    ) {
        Some(UserFacingError {
            code: "openvpn_options_error".into(),
            title: "VPN profile was rejected".into(),
            message: exit_message(
                code,
                "OpenVPN rejected one or more profile directives or command options.",
            ),
            suggested_fix: Some(
                "Review the imported profile for unsupported or invalid directives.".into(),
            ),
            details_safe,
        })
    } else if contains_any(
        &detail,
        &[
            "tls error",
            "verify error",
            "certificate",
            "tls-crypt",
            "tls-auth",
        ],
    ) {
        Some(UserFacingError {
            code: "openvpn_tls_failed".into(),
            title: "TLS handshake failed".into(),
            message: exit_message(
                code,
                "OpenVPN could not complete the TLS handshake with the server.",
            ),
            suggested_fix: Some(
                "Check the server certificate, client credentials, and the profile's TLS settings."
                    .into(),
            ),
            details_safe,
        })
    } else if contains_any(
        &detail,
        &[
            "cannot open tun/tap dev",
            "operation not permitted",
            "route addition failed",
            "permission denied",
        ],
    ) {
        Some(UserFacingError {
            code: "openvpn_permission_failed".into(),
            title: "OpenVPN lacked system permissions".into(),
            message: exit_message(
                code,
                "OpenVPN started but could not configure the tunnel or routes.",
            ),
            suggested_fix: Some(
                "Verify the privileged helper setup and check that OpenWrap has the required macOS permissions."
                    .into(),
            ),
            details_safe,
        })
    } else if contains_any(
        &detail,
        &[
            "connection refused",
            "network is unreachable",
            "no route to host",
            "network unreachable",
            "connection reset",
            "connection timed out",
            "transport error",
        ],
    ) {
        Some(UserFacingError {
            code: "openvpn_network_failed".into(),
            title: "Network connection to the VPN server failed".into(),
            message: exit_message(
                code,
                "OpenVPN could not establish or keep the transport connection to the server.",
            ),
            suggested_fix: Some(
                "Confirm the server is reachable and that no firewall or local network policy is blocking the connection."
                    .into(),
            ),
            details_safe,
        })
    } else {
        Some(UserFacingError {
            code: "process_exit".into(),
            title: "Connection failed".into(),
            message: exit_message(code, "OpenVPN exited after reporting a connection error."),
            suggested_fix: Some(
                "Review the OpenVPN detail below, then use Show logs to inspect the last OpenVPN output."
                    .into(),
            ),
            details_safe,
        })
    }
}

fn error_classification(line: &str) -> Option<(LogLevel, &'static str)> {
    if contains_any(
        line,
        &["options error", "unrecognized option", "unknown option"],
    ) {
        Some((LogLevel::Error, "config_error"))
    } else if contains_any(
        line,
        &[
            "cannot resolve host address",
            "temporary failure in name resolution",
        ],
    ) {
        Some((LogLevel::Error, "dns_resolution"))
    } else if contains_any(
        line,
        &[
            "tls error",
            "verify error",
            "certificate",
            "tls-crypt",
            "tls-auth",
        ],
    ) {
        Some((LogLevel::Error, "tls_error"))
    } else if contains_any(
        line,
        &[
            "cannot open tun/tap dev",
            "operation not permitted",
            "route addition failed",
            "permission denied",
        ],
    ) {
        Some((LogLevel::Error, "permission_error"))
    } else if contains_any(
        line,
        &[
            "connection refused",
            "network is unreachable",
            "no route to host",
            "network unreachable",
            "connection reset",
            "connection timed out",
            "transport error",
        ],
    ) {
        Some((LogLevel::Warn, "network_error"))
    } else if line.contains("fatal error") {
        Some((LogLevel::Error, "fatal_error"))
    } else {
        None
    }
}

fn relevant_failure_detail(entry: &LogEntry) -> Option<String> {
    let message = entry.message.trim();
    if message.is_empty() || is_noise(message) {
        return None;
    }

    if matches!(entry.level, LogLevel::Error | LogLevel::Warn) || looks_like_failure(message) {
        Some(truncate_detail(message))
    } else {
        None
    }
}

fn looks_like_failure(message: &str) -> bool {
    contains_any(
        message,
        &[
            "error",
            "failed",
            "fatal",
            "cannot",
            "unable",
            "denied",
            "rejected",
            "refused",
            "unreachable",
            "timed out",
        ],
    )
}

fn is_noise(message: &str) -> bool {
    contains_any(
        message,
        &[
            "exiting due to fatal error",
            "process exited",
            "restart pause",
            "sigterm",
            "sigusr1",
        ],
    )
}

fn contains_any(message: &str, needles: &[&str]) -> bool {
    let lower = message.to_ascii_lowercase();
    needles.iter().any(|needle| lower.contains(needle))
}

fn truncate_detail(message: &str) -> String {
    const MAX_LEN: usize = 180;

    let trimmed = message.trim();
    if trimmed.chars().count() <= MAX_LEN {
        trimmed.to_string()
    } else {
        let shortened = trimmed.chars().take(MAX_LEN - 3).collect::<String>();
        format!("{shortened}...")
    }
}

fn exit_message(code: Option<i32>, summary: &str) -> String {
    match code {
        Some(code) => format!("{summary} OpenVPN exited with status {code}."),
        None => format!("{summary} OpenVPN terminated without reporting an exit status."),
    }
}

#[cfg(test)]
mod tests {
    use crate::connection::session::LogLevel;

    use super::{classify_signal, diagnose_exit_error, sanitize_log, ParsedLogSignal};

    #[test]
    fn redacts_sensitive_markers() {
        let entry = sanitize_log("stderr", "AUTH_FAILED password mismatch");
        assert!(entry.sanitized);
        assert!(!entry.message.contains("password"));
    }

    #[test]
    fn classifies_dns_hints() {
        assert_eq!(
            classify_signal("PUSH_REPLY,dhcp-option DNS 1.1.1.1"),
            ParsedLogSignal::DnsHint
        );
    }

    #[test]
    fn classifies_common_openvpn_errors() {
        let entry = sanitize_log(
            "stderr",
            "Options error: Unrecognized option or missing parameter(s)",
        );
        assert_eq!(entry.level, LogLevel::Error);
        assert_eq!(entry.classification, "config_error");
    }

    #[test]
    fn diagnoses_hostname_resolution_failures() {
        let logs = vec![
            sanitize_log("stdout", "TCP/UDP: Preserving recently used remote address"),
            sanitize_log(
                "stderr",
                "RESOLVE: Cannot resolve host address: vpn.example.invalid:1194 (nodename nor servname provided, or not known)",
            ),
        ];

        let error = diagnose_exit_error(Some(1), logs.iter()).expect("expected diagnosis");

        assert_eq!(error.code, "openvpn_host_resolution_failed");
        assert_eq!(error.title, "Server hostname could not be resolved");
        assert!(error
            .details_safe
            .as_deref()
            .is_some_and(|detail| detail.contains("Cannot resolve host address")));
    }

    #[test]
    fn falls_back_to_last_relevant_failure_detail() {
        let logs = vec![
            sanitize_log("stdout", "OpenVPN 2.6.0 x86_64-apple-darwin"),
            sanitize_log("stderr", "helper: profile path was rejected by policy"),
            sanitize_log("stderr", "Exiting due to fatal error"),
        ];

        let error = diagnose_exit_error(Some(1), logs.iter()).expect("expected diagnosis");

        assert_eq!(error.code, "process_exit");
        assert!(error
            .details_safe
            .as_deref()
            .is_some_and(|detail| detail.contains("profile path was rejected by policy")));
    }
}
