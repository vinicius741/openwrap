//! Report assembly for profile imports.
//!
//! This module handles building and populating import reports.

use crate::profiles::{ImportReport, ValidationAction, ValidationFinding};

/// Builder for creating import reports with a fluent API.
#[derive(Debug, Clone, Default)]
pub struct ReportBuilder {
    report: ImportReport,
}

impl ReportBuilder {
    pub fn new() -> Self {
        Self {
            report: ImportReport::default(),
        }
    }

    /// Adds validation findings to the report.
    ///
    /// This separates findings into blocked directives and warnings,
    /// and generates error messages for blocked items.
    pub fn with_findings(mut self, findings: &[ValidationFinding]) -> Self {
        let blocked: Vec<_> = findings
            .iter()
            .filter(|finding| finding.action == ValidationAction::Block)
            .cloned()
            .collect();

        let warnings: Vec<_> = findings
            .iter()
            .filter(|finding| finding.action == ValidationAction::RequireApproval)
            .cloned()
            .collect();

        self.report.blocked_directives = blocked.clone();
        self.report.warnings = warnings;

        // Generate error messages for blocked items
        for finding in &blocked {
            self.report.errors.push(format!(
                "Line {}: {}",
                finding.line, finding.message
            ));
        }

        self
    }

    /// Builds the final import report.
    pub fn build(self) -> ImportReport {
        self.report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::ValidationSeverity;

    #[test]
    fn builds_report_with_findings() {
        let findings = vec![
            ValidationFinding {
                severity: ValidationSeverity::Error,
                directive: "up".to_string(),
                line: 5,
                message: "Blocked directive".to_string(),
                action: ValidationAction::Block,
            },
            ValidationFinding {
                severity: ValidationSeverity::Warn,
                directive: "pull-filter".to_string(),
                line: 6,
                message: "Requires approval".to_string(),
                action: ValidationAction::RequireApproval,
            },
        ];

        let report = ReportBuilder::new()
            .with_findings(&findings)
            .build();

        assert_eq!(report.blocked_directives.len(), 1);
        assert_eq!(report.warnings.len(), 1);
        assert_eq!(report.errors.len(), 1);
        assert!(report.errors[0].contains("Line 5"));
    }
}
