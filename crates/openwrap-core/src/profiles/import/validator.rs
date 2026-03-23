//! Pure validation logic for profile imports.
//!
//! This module handles directive classification and validation finding generation
//! without any filesystem operations.

use crate::config::{classify_directive, DirectiveClassification};
use crate::profiles::{ParsedDirective, ValidationAction, ValidationFinding, ValidationSeverity};

/// Result of validating profile directives.
#[derive(Debug, Clone, Default)]
pub struct ValidationResult {
    /// Findings that block import entirely.
    pub blocked: Vec<ValidationFinding>,
    /// Findings that require user approval.
    pub warnings: Vec<ValidationFinding>,
}

/// Validates parsed profile directives for import eligibility.
///
/// This function classifies each directive and generates appropriate findings
/// for blocked or warned directives. It also handles special cases like
/// `auth-user-pass` with file paths.
pub fn validate_directives(directives: &[ParsedDirective]) -> ValidationResult {
    let mut blocked = Vec::new();
    let mut warnings = Vec::new();

    for directive in directives {
        // Special handling for auth-user-pass with file paths
        if directive.name == "auth-user-pass" && !directive.args.is_empty() {
            blocked.push(ValidationFinding {
                severity: ValidationSeverity::Error,
                directive: directive.name.clone(),
                line: directive.line,
                message: "auth-user-pass file paths are blocked in v1; use Keychain-backed prompts instead.".into(),
                action: ValidationAction::Block,
            });
            continue;
        }

        let classification = classify_directive(&directive.name, &directive.args);
        match classification {
            DirectiveClassification::Allowed => {}
            DirectiveClassification::Warned => warnings.push(ValidationFinding {
                severity: ValidationSeverity::Warn,
                directive: directive.name.clone(),
                line: directive.line,
                message: format!(
                    "'{}' requires explicit approval during import.",
                    directive.name
                ),
                action: ValidationAction::RequireApproval,
            }),
            DirectiveClassification::Blocked => blocked.push(ValidationFinding {
                severity: ValidationSeverity::Error,
                directive: directive.name.clone(),
                line: directive.line,
                message: format!("'{}' is blocked in v1.", directive.name),
                action: ValidationAction::Block,
            }),
        }
    }

    ValidationResult { blocked, warnings }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_directive(name: &str, args: &[&str], line: usize) -> ParsedDirective {
        ParsedDirective {
            name: name.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            line,
        }
    }

    #[test]
    fn allows_standard_directives() {
        let directives = vec![
            make_directive("client", &[], 1),
            make_directive("remote", &["vpn.example.com", "1194"], 2),
            make_directive("ca", &["ca.crt"], 3),
        ];

        let result = validate_directives(&directives);
        assert!(result.blocked.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn blocks_auth_user_pass_with_file() {
        let directives = vec![
            make_directive("client", &[], 1),
            make_directive("auth-user-pass", &["creds.txt"], 2),
        ];

        let result = validate_directives(&directives);
        assert_eq!(result.blocked.len(), 1);
        assert_eq!(result.blocked[0].directive, "auth-user-pass");
        assert_eq!(result.blocked[0].line, 2);
    }

    #[test]
    fn warns_on_warned_directives() {
        let directives = vec![
            make_directive("client", &[], 1),
            make_directive("pull-filter", &["ignore", "redirect-gateway"], 2),
        ];

        let result = validate_directives(&directives);
        assert!(result.blocked.is_empty());
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.warnings[0].directive, "pull-filter");
    }

    #[test]
    fn blocks_blocked_directives() {
        let directives = vec![
            make_directive("client", &[], 1),
            make_directive("up", &["script.sh"], 2),
        ];

        let result = validate_directives(&directives);
        assert_eq!(result.blocked.len(), 1);
        assert_eq!(result.blocked[0].directive, "up");
    }
}
