use crate::config::{classify_directive, DirectiveClassification};
use crate::profiles::{ParsedProfile, ValidationAction, ValidationFinding, ValidationSeverity};

pub fn validate_directives(parsed: &ParsedProfile) -> Vec<ValidationFinding> {
    let mut findings = Vec::new();

    for directive in &parsed.directives {
        if directive.name == "auth-user-pass" && !directive.args.is_empty() {
            findings.push(ValidationFinding {
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
            DirectiveClassification::Warned => findings.push(ValidationFinding {
                severity: ValidationSeverity::Warn,
                directive: directive.name.clone(),
                line: directive.line,
                message: format!(
                    "'{}' requires explicit approval during import.",
                    directive.name
                ),
                action: ValidationAction::RequireApproval,
            }),
            DirectiveClassification::Blocked => findings.push(ValidationFinding {
                severity: ValidationSeverity::Error,
                directive: directive.name.clone(),
                line: directive.line,
                message: format!("'{}' is blocked in v1.", directive.name),
                action: ValidationAction::Block,
            }),
        }
    }

    findings
}

pub fn blocked_findings(findings: &[ValidationFinding]) -> Vec<ValidationFinding> {
    findings
        .iter()
        .filter(|f| f.action == ValidationAction::Block)
        .cloned()
        .collect()
}

pub fn warning_findings(findings: &[ValidationFinding]) -> Vec<ValidationFinding> {
    findings
        .iter()
        .filter(|f| f.action == ValidationAction::RequireApproval)
        .cloned()
        .collect()
}
