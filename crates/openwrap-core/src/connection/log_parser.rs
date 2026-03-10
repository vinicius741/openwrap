use chrono::Utc;

use crate::connection::session::{LogEntry, LogLevel};

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
    } else if line.contains("dhcp-option DNS") || line.contains("PUSH_REPLY") {
        (LogLevel::Info, "dns")
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
    } else if line.contains("dhcp-option DNS") || line.contains("PUSH_REPLY") {
        ParsedLogSignal::DnsHint
    } else {
        ParsedLogSignal::None
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_signal, sanitize_log, ParsedLogSignal};

    #[test]
    fn redacts_sensitive_markers() {
        let entry = sanitize_log("stderr", "AUTH_FAILED password mismatch");
        assert!(entry.sanitized);
        assert!(!entry.message.contains("password"));
    }

    #[test]
    fn classifies_dns_hints() {
        assert_eq!(classify_signal("PUSH_REPLY,dhcp-option DNS 1.1.1.1"), ParsedLogSignal::DnsHint);
    }
}

