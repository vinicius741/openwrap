use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ValidationStatus {
    Ok,
    Warning,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ValidationSeverity {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ValidationAction {
    Allow,
    RequireApproval,
    Block,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationFinding {
    pub severity: ValidationSeverity,
    pub directive: String,
    pub line: usize,
    pub message: String,
    pub action: ValidationAction,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_status_serialization() {
        let statuses = vec![
            ValidationStatus::Ok,
            ValidationStatus::Warning,
            ValidationStatus::Blocked,
        ];
        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let roundtrip: ValidationStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, roundtrip);
        }
    }

    #[test]
    fn validation_severity_order() {
        assert_eq!(
            serde_json::to_string(&ValidationSeverity::Info).unwrap(),
            "\"Info\""
        );
        assert_eq!(
            serde_json::to_string(&ValidationSeverity::Warn).unwrap(),
            "\"Warn\""
        );
        assert_eq!(
            serde_json::to_string(&ValidationSeverity::Error).unwrap(),
            "\"Error\""
        );
    }

    #[test]
    fn validation_action_serialization() {
        let actions = vec![
            ValidationAction::Allow,
            ValidationAction::RequireApproval,
            ValidationAction::Block,
        ];
        for action in actions {
            let json = serde_json::to_string(&action).unwrap();
            let roundtrip: ValidationAction = serde_json::from_str(&json).unwrap();
            assert_eq!(action, roundtrip);
        }
    }

    #[test]
    fn validation_finding_structure() {
        let finding = ValidationFinding {
            severity: ValidationSeverity::Error,
            directive: "script-security".to_string(),
            line: 42,
            message: "Blocked directive".to_string(),
            action: ValidationAction::Block,
        };
        assert_eq!(finding.severity, ValidationSeverity::Error);
        assert_eq!(finding.directive, "script-security");
        assert_eq!(finding.line, 42);
        assert_eq!(finding.action, ValidationAction::Block);
    }
}
