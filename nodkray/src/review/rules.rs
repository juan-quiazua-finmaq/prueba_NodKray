//! Local rule registration for RDD FAST (spec §31, §34).
//!
//! Phase 2 only registers which rule files are present; it does not analyse
//! their contents. Deeper Sentrux/rule validation arrives in phase 5.

use std::path::Path;

use serde::Serialize;

use crate::installer::project::detect_rules;

/// Outcome of the rules check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RulesCheck {
    pub status: String,
    pub detail: String,
    pub rules: Vec<String>,
}

/// Detect project rule files in `dir` and report them without interpretation.
pub fn check_rules(dir: &Path) -> RulesCheck {
    let rules: Vec<String> = detect_rules(dir)
        .into_iter()
        .map(|rule| rule.name)
        .collect();
    let detail = if rules.is_empty() {
        "no project rule files detected".to_string()
    } else {
        format!("{} rule file(s) detected: {}", rules.len(), rules.join(", "))
    };
    RulesCheck {
        status: "passed".to_string(),
        detail,
        rules,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_detected_rules_without_analysis() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(tmp.path().join("AGENTS.md"), "rules").expect("write");
        let check = check_rules(tmp.path());
        assert_eq!(check.status, "passed");
        assert!(check.rules.contains(&"AGENTS.md".to_string()));
    }
}
