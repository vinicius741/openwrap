use serde::{Deserialize, Serialize};

use crate::profiles::{ValidationAction, ValidationFinding, ValidationSeverity};

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
        | "tls-client"
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
        | "explicit-exit-notify"
        | "reneg-sec"
        | "key-direction"
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
        "redirect-gateway" | "route-nopull" | "route" | "pull-filter" => {
            DirectiveClassification::Warned
        }
        "dhcp-option" => match args.first().map(|value| value.to_ascii_uppercase()) {
            Some(option) if option == "DNS" || option == "DOMAIN" || option == "DOMAIN-SEARCH" => {
                DirectiveClassification::Warned
            }
            _ => DirectiveClassification::Blocked,
        },
        "setenv" => match (args.first(), args.get(1)) {
            (Some(name), Some(value))
                if name.eq_ignore_ascii_case("CLIENT_CERT") && value == "0" =>
            {
                DirectiveClassification::Allowed
            }
            _ => DirectiveClassification::Blocked,
        },
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
            message: format!(
                "'{name}' changes routing or environment behavior and needs approval."
            ),
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
        assert_eq!(
            classify_directive("client", &[]),
            DirectiveClassification::Allowed
        );
        assert_eq!(
            classify_directive("tls-client", &[]),
            DirectiveClassification::Allowed
        );
        assert_eq!(
            classify_directive("explicit-exit-notify", &[String::from("1")]),
            DirectiveClassification::Allowed
        );
        assert_eq!(
            classify_directive("reneg-sec", &[String::from("0")]),
            DirectiveClassification::Allowed
        );
        assert_eq!(
            classify_directive("key-direction", &[String::from("1")]),
            DirectiveClassification::Allowed
        );
        assert_eq!(
            classify_directive("redirect-gateway", &[]),
            DirectiveClassification::Warned
        );
        assert_eq!(
            classify_directive(
                "route",
                &[String::from("10.0.0.1"), String::from("255.255.255.255")]
            ),
            DirectiveClassification::Warned
        );
        assert_eq!(
            classify_directive(
                "pull-filter",
                &[String::from("ignore"), String::from("redirect-gateway"),]
            ),
            DirectiveClassification::Warned
        );
        assert_eq!(
            classify_directive(
                "dhcp-option",
                &[String::from("DNS"), String::from("1.1.1.1")]
            ),
            DirectiveClassification::Warned
        );
        assert_eq!(
            classify_directive(
                "dhcp-option",
                &[String::from("DOMAIN"), String::from("corp.example")]
            ),
            DirectiveClassification::Warned
        );
        assert_eq!(
            classify_directive(
                "dhcp-option",
                &[
                    String::from("DOMAIN-SEARCH"),
                    String::from("corp.example"),
                    String::from("lab.example")
                ]
            ),
            DirectiveClassification::Warned
        );
        assert_eq!(
            classify_directive("setenv", &[String::from("CLIENT_CERT"), String::from("0")]),
            DirectiveClassification::Allowed
        );
        assert_eq!(
            classify_directive("plugin", &[]),
            DirectiveClassification::Blocked
        );
        assert_eq!(
            classify_directive("script-security", &[String::from("2")]),
            DirectiveClassification::Blocked
        );
        assert_eq!(
            classify_directive(
                "dhcp-option",
                &[String::from("NTP"), String::from("corp.example")]
            ),
            DirectiveClassification::Blocked
        );
        assert_eq!(
            classify_directive("setenv", &[String::from("FOO"), String::from("bar")]),
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
