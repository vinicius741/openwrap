use serde::{Deserialize, Serialize};

use crate::profiles::{
    ValidationAction, ValidationFinding, ValidationSeverity,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DirectiveClassification {
    Allowed,
    Warned,
    Blocked,
}

pub fn classify_directive(name: &str, args: &[String]) -> DirectiveClassification {
    match name {
        "client"
        | "dev"
        | "proto"
        | "remote"
        | "remote-random"
        | "nobind"
        | "resolv-retry"
        | "persist-key"
        | "persist-tun"
        | "remote-cert-tls"
        | "verify-x509-name"
        | "cipher"
        | "data-ciphers"
        | "data-ciphers-fallback"
        | "auth"
        | "ca"
        | "cert"
        | "key"
        | "pkcs12"
        | "tls-auth"
        | "tls-crypt"
        | "auth-user-pass"
        | "verb"
        | "mute"
        | "auth-nocache" => DirectiveClassification::Allowed,
        "redirect-gateway" | "route-nopull" => DirectiveClassification::Warned,
        "dhcp-option" => match args.first().map(|value| value.to_ascii_uppercase()) {
            Some(option) if option == "DNS" => DirectiveClassification::Warned,
            _ => DirectiveClassification::Blocked,
        },
        "setenv" => DirectiveClassification::Blocked,
        "script-security" => match args.first().and_then(|value| value.parse::<u8>().ok()) {
            Some(level) if level <= 1 => DirectiveClassification::Allowed,
            _ => DirectiveClassification::Blocked,
        },
        "up"
        | "down"
        | "route-up"
        | "route-pre-down"
        | "client-connect"
        | "client-disconnect"
        | "plugin"
        | "ipchange"
        | "learn-address"
        | "management"
        | "management-client"
        | "management-query-passwords"
        | "management-hold"
        | "daemon"
        | "cd"
        | "log"
        | "log-append"
        | "status"
        | "machine-readable-output" => DirectiveClassification::Blocked,
        _ => DirectiveClassification::Blocked,
    }
}

pub fn finding_for(
    name: &str,
    line: usize,
    classification: DirectiveClassification,
) -> Option<ValidationFinding> {
    match classification {
        DirectiveClassification::Allowed => None,
        DirectiveClassification::Warned => Some(ValidationFinding {
            severity: ValidationSeverity::Warn,
            directive: name.to_string(),
            line,
            message: format!("'{name}' changes routing or environment behavior and needs approval."),
            action: ValidationAction::RequireApproval,
        }),
        DirectiveClassification::Blocked => Some(ValidationFinding {
            severity: ValidationSeverity::Error,
            directive: name.to_string(),
            line,
            message: format!("'{name}' is blocked in v1 because it can escape app-managed policy."),
            action: ValidationAction::Block,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_directive, DirectiveClassification};

    #[test]
    fn classifies_known_directives() {
        assert_eq!(classify_directive("client", &[]), DirectiveClassification::Allowed);
        assert_eq!(
            classify_directive("redirect-gateway", &[]),
            DirectiveClassification::Warned
        );
        assert_eq!(
            classify_directive("dhcp-option", &[String::from("DNS"), String::from("1.1.1.1")]),
            DirectiveClassification::Warned
        );
        assert_eq!(classify_directive("plugin", &[]), DirectiveClassification::Blocked);
        assert_eq!(
            classify_directive("script-security", &[String::from("2")]),
            DirectiveClassification::Blocked
        );
        assert_eq!(
            classify_directive("dhcp-option", &[String::from("DOMAIN"), String::from("corp.example")]),
            DirectiveClassification::Blocked
        );
    }

    #[test]
    fn blocks_unknown_directives() {
        assert_eq!(
            classify_directive("mystery-directive", &[]),
            DirectiveClassification::Blocked
        );
    }
}
