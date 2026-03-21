use std::fs;
use std::path::Path;

use super::model::{CategoryScore, LintError, LintIssue, LintReport, LintRules, Spec};

pub struct SpecLinter {
    rules: LintRules,
}

impl SpecLinter {
    pub fn new(rules_path: &Path) -> Result<Self, LintError> {
        let rules_content = fs::read_to_string(rules_path)?;
        let rules: LintRules = serde_yaml::from_str(&rules_content)?;
        Self::validate_rules(&rules)?;
        Ok(Self { rules })
    }

    fn validate_rules(rules: &LintRules) -> Result<(), LintError> {
        const ALLOWED_RULE_IDS: [&str; 10] = [
            "SPEC-001", "SPEC-002", "SPEC-003", "SPEC-004", "SPEC-010", "SPEC-011", "SPEC-020",
            "SPEC-021", "SPEC-030", "SPEC-040",
        ];

        for rule in &rules.rules {
            let rule_id = rule.id.trim();
            if rule_id.is_empty() {
                return Err(LintError::MissingRequiredField {
                    rule_id: "<unknown>".to_string(),
                    field: "id".to_string(),
                });
            }

            if !ALLOWED_RULE_IDS.contains(&rule_id) {
                return Err(LintError::UnknownRuleId {
                    rule_id: rule_id.to_string(),
                });
            }

            if rule.name.trim().is_empty() {
                return Err(LintError::MissingRequiredField {
                    rule_id: rule.id.clone(),
                    field: "name".to_string(),
                });
            }

            if rule.description.trim().is_empty() {
                return Err(LintError::MissingRequiredField {
                    rule_id: rule.id.clone(),
                    field: "description".to_string(),
                });
            }

            let severity = rule.severity.trim();
            if severity.is_empty() {
                return Err(LintError::MissingRequiredField {
                    rule_id: rule.id.clone(),
                    field: "severity".to_string(),
                });
            }

            if severity != "error" && severity != "warning" {
                return Err(LintError::InvalidSeverity {
                    rule_id: rule.id.clone(),
                    severity: rule.severity.clone(),
                });
            }
        }

        Ok(())
    }

    pub fn lint(&self, spec_path: &Path) -> Result<LintReport, LintError> {
        let spec_content = fs::read_to_string(spec_path)?;
        let spec: Spec = serde_yaml::from_str(&spec_content)?;

        let mut report = LintReport::new(
            spec.specification.identity.id.clone(),
            spec.specification.identity.version.clone(),
        );

        Self::check_completeness(&self.rules, &spec, &mut report);
        Self::check_clarity(&self.rules, &spec, &mut report);
        Self::check_security(&self.rules, &spec, &mut report);
        Self::check_testability(&self.rules, &spec, &mut report);
        Self::check_data_model(&self.rules, &spec, &mut report);

        report.calculate_score();
        Ok(report)
    }

    fn check_completeness(rules: &LintRules, spec: &Spec, report: &mut LintReport) {
        let spec_001_rule = rules.rules.iter().find(|r| r.id == "SPEC-001");
        let spec_003_rule = rules.rules.iter().find(|r| r.id == "SPEC-003");
        let spec_004_rule = rules.rules.iter().find(|r| r.id == "SPEC-004");
        let spec_011_rule = rules.rules.iter().find(|r| r.id == "SPEC-011");

        let spec_001_severity = spec_001_rule
            .map(|r| r.severity.clone())
            .unwrap_or_else(|| "error".to_string());
        let spec_003_severity = spec_003_rule
            .map(|r| r.severity.clone())
            .unwrap_or_else(|| "error".to_string());
        let spec_004_severity = spec_004_rule
            .map(|r| r.severity.clone())
            .unwrap_or_else(|| "warning".to_string());
        let spec_011_severity = spec_011_rule
            .map(|r| r.severity.clone())
            .unwrap_or_else(|| "error".to_string());

        let mut error_count = 0;
        let mut warning_count = 0;

        if let Some(rule) = spec_001_rule {
            for dep in spec
                .specification
                .context
                .system_dependencies
                .iter()
                .filter(|dep| dep.twin_available)
            {
                let has_error_handling = spec.specification.behaviors.iter().any(|behavior| {
                    behavior.edge_cases.as_ref().is_some_and(|edge_cases| {
                        edge_cases.iter().any(|edge_case| {
                            edge_case.then.iter().any(|then_clause| {
                                let then_lower = then_clause.to_lowercase();
                                let dep_lower = dep.service.to_lowercase();
                                then_lower.contains(&dep_lower)
                                    && (then_lower.contains("fail")
                                        || then_lower.contains("error")
                                        || then_lower.contains("unavailable"))
                            })
                        })
                    })
                });

                if !has_error_handling {
                    if spec_001_severity == "error" {
                        report.errors.push(LintIssue {
                            rule_id: rule.id.clone(),
                            rule_name: rule.name.clone(),
                            severity: spec_001_severity.clone(),
                            message: format!(
                                "Dependency '{}' has no error handling edge case",
                                dep.service
                            ),
                            line: None,
                        });
                        error_count += 1;
                    } else {
                        report.warnings.push(LintIssue {
                            rule_id: rule.id.clone(),
                            rule_name: rule.name.clone(),
                            severity: spec_001_severity.clone(),
                            message: format!(
                                "Dependency '{}' has no error handling edge case",
                                dep.service
                            ),
                            line: None,
                        });
                        warning_count += 1;
                    }
                }
            }
        }

        if let Some(rule) = spec_003_rule {
            if let Some(contract) = &spec.specification.api_contract {
                if let Some(endpoints) = &contract.endpoints {
                    for endpoint in endpoints.iter().filter(|e| e.authentication.is_none()) {
                        if spec_003_severity == "error" {
                            report.errors.push(LintIssue {
                                rule_id: rule.id.clone(),
                                rule_name: rule.name.clone(),
                                severity: spec_003_severity.clone(),
                                message: format!(
                                    "Endpoint {} {} missing authentication specification",
                                    endpoint.method, endpoint.path
                                ),
                                line: None,
                            });
                            error_count += 1;
                        } else {
                            report.warnings.push(LintIssue {
                                rule_id: rule.id.clone(),
                                rule_name: rule.name.clone(),
                                severity: spec_003_severity.clone(),
                                message: format!(
                                    "Endpoint {} {} missing authentication specification",
                                    endpoint.method, endpoint.path
                                ),
                                line: None,
                            });
                            warning_count += 1;
                        }
                    }
                }
            }
        }

        if let Some(rule) = spec_004_rule {
            for behavior in &spec.specification.behaviors {
                let has_criterion = spec.specification.acceptance_criteria.iter().any(|ac| {
                    ac.behavior_ref
                        .as_ref()
                        .is_some_and(|ref_id| ref_id == &behavior.id)
                });

                if !has_criterion {
                    if spec_004_severity == "error" {
                        report.errors.push(LintIssue {
                            rule_id: rule.id.clone(),
                            rule_name: rule.name.clone(),
                            severity: spec_004_severity.clone(),
                            message: format!(
                                "Behavior '{}' has no acceptance criterion",
                                behavior.id
                            ),
                            line: None,
                        });
                        error_count += 1;
                    } else {
                        report.warnings.push(LintIssue {
                            rule_id: rule.id.clone(),
                            rule_name: rule.name.clone(),
                            severity: spec_004_severity.clone(),
                            message: format!(
                                "Behavior '{}' has no acceptance criterion",
                                behavior.id
                            ),
                            line: None,
                        });
                        warning_count += 1;
                    }
                }
            }
        }

        if let Some(rule) = spec_011_rule {
            let error_terms = [
                "error",
                "fail",
                "exception",
                "invalid",
                "unauthorized",
                "not found",
            ];
            for behavior in &spec.specification.behaviors {
                let mentions_error = behavior.then.iter().any(|t| {
                    error_terms
                        .iter()
                        .any(|term| t.to_lowercase().contains(term))
                });

                let has_concrete_response = behavior.then.iter().any(|t| {
                    t.contains("HTTP")
                        || t.contains("status")
                        || t.contains("code")
                        || t.contains("response")
                        || t.contains("400")
                        || t.contains("500")
                        || t.contains("401")
                        || t.contains("404")
                });

                if mentions_error && !has_concrete_response {
                    if spec_011_severity == "error" {
                        report.errors.push(LintIssue {
                            rule_id: rule.id.clone(),
                            rule_name: rule.name.clone(),
                            severity: spec_011_severity.clone(),
                            message: format!("Behavior '{}' mentions error but doesn't specify concrete HTTP status code", behavior.id),
                            line: None,
                        });
                        error_count += 1;
                    } else {
                        report.warnings.push(LintIssue {
                            rule_id: rule.id.clone(),
                            rule_name: rule.name.clone(),
                            severity: spec_011_severity.clone(),
                            message: format!("Behavior '{}' mentions error but doesn't specify concrete HTTP status code", behavior.id),
                            line: None,
                        });
                        warning_count += 1;
                    }
                }
            }
        }

        let total: usize = error_count + warning_count;
        let passed = if error_count > 0 || warning_count > 0 {
            0
        } else {
            1
        };
        let score = if total > 0 {
            (passed * 100) / total
        } else {
            100
        };
        let score: u32 = score.try_into().map_or(100, |score| score);
        report.categories.insert(
            "Completeness".to_string(),
            CategoryScore {
                score,
                details: format!("{}/{} rules passed", passed, total),
            },
        );
    }

    fn check_clarity(rules: &LintRules, spec: &Spec, report: &mut LintReport) {
        let spec_010_rule = rules.rules.iter().find(|r| r.id == "SPEC-010");
        let banned: Vec<&str> = spec_010_rule
            .and_then(|r| r.banned_phrases.as_ref())
            .map(|phrases| phrases.iter().map(|s| s.as_str()).collect())
            .unwrap_or_else(Vec::new);

        let severity = spec_010_rule
            .map(|r| r.severity.clone())
            .unwrap_or_else(|| "warning".to_string());

        let issues: Vec<_> = spec
            .specification
            .behaviors
            .iter()
            .flat_map(|behavior| {
                behavior.then.iter().filter_map(|then_clause| {
                    banned.iter().find_map(|phrase| {
                        if then_clause.to_lowercase().contains(phrase) {
                            Some(LintIssue {
                                rule_id: "SPEC-010".to_string(),
                                rule_name: "no-ambiguous-language".to_string(),
                                severity: severity.clone(),
                                message: format!(
                                    "Found ambiguous phrase: '{phrase}' in behavior {}",
                                    behavior.id
                                ),
                                line: None,
                            })
                        } else {
                            None
                        }
                    })
                })
            })
            .collect();

        issues.iter().for_each(|issue| {
            if issue.severity == "error" {
                report.errors.push(issue.clone());
            } else {
                report.warnings.push(issue.clone());
            }
        });

        let score = if issues.is_empty() { 100 } else { 88 };
        report.categories.insert(
            "Clarity".to_string(),
            CategoryScore {
                score,
                details: format!("{} ambiguous phrases found", issues.len()),
            },
        );
    }

    fn check_security(rules: &LintRules, spec: &Spec, report: &mut LintReport) {
        let spec_020_rule = rules.rules.iter().find(|r| r.id == "SPEC-020");
        let spec_021_rule = rules.rules.iter().find(|r| r.id == "SPEC-021");
        let spec_040_rule = rules.rules.iter().find(|r| r.id == "SPEC-040");

        let spec_020_severity = spec_020_rule
            .map(|r| r.severity.clone())
            .unwrap_or_else(|| "error".to_string());
        let spec_021_severity = spec_021_rule
            .map(|r| r.severity.clone())
            .unwrap_or_else(|| "warning".to_string());
        let spec_040_severity = spec_040_rule
            .map(|r| r.severity.clone())
            .unwrap_or_else(|| "warning".to_string());

        if let Some(contract) = &spec.specification.api_contract {
            if let Some(endpoints) = &contract.endpoints {
                let user_endpoints: Vec<_> = endpoints
                    .iter()
                    .filter(|e| {
                        e.path.contains("email")
                            || e.path.contains("username")
                            || e.path.contains("password")
                    })
                    .collect();

                for endpoint in &user_endpoints {
                    let has_enumeration_check = spec.specification.behaviors.iter().any(|b| {
                        b.edge_cases.as_ref().is_some_and(|ec| {
                            ec.iter().any(|e| {
                                e.id.contains("nonexist")
                                    || e.id.contains("not-found")
                                    || e.id.contains("enumeration")
                            })
                        })
                    });

                    if !has_enumeration_check {
                        let issue = LintIssue {
                            rule_id: "SPEC-020".to_string(),
                            rule_name: "enumeration-prevention".to_string(),
                            severity: spec_020_severity.clone(),
                            message: format!(
                                "Endpoint {} may be vulnerable to user enumeration",
                                endpoint.path
                            ),
                            line: None,
                        };
                        if issue.severity == "error" {
                            report.errors.push(issue);
                        } else {
                            report.warnings.push(issue);
                        }
                    }
                }

                let write_methods = ["POST", "PUT", "PATCH", "DELETE"];
                let has_write_endpoints = endpoints
                    .iter()
                    .any(|e| write_methods.contains(&e.method.as_str()));
                if has_write_endpoints {
                    let has_rate_limit = spec.specification.behaviors.iter().any(|b| {
                        b.then.iter().any(|t| {
                            t.to_lowercase().contains("rate")
                                || t.to_lowercase().contains("throttle")
                                || t.to_lowercase().contains("limit")
                        }) || b.edge_cases.as_ref().is_some_and(|ec| {
                            ec.iter().any(|e| {
                                e.then.iter().any(|t| {
                                    t.to_lowercase().contains("rate")
                                        || t.to_lowercase().contains("throttle")
                                        || t.to_lowercase().contains("limit")
                                })
                            })
                        })
                    });

                    if !has_rate_limit {
                        let issue = LintIssue {
                            rule_id: "SPEC-021".to_string(),
                            rule_name: "rate-limiting-specified".to_string(),
                            severity: spec_021_severity.clone(),
                            message:
                                "Write endpoints found but no rate limiting behavior specified"
                                    .to_string(),
                            line: None,
                        };
                        if issue.severity == "error" {
                            report.errors.push(issue);
                        } else {
                            report.warnings.push(issue);
                        }
                    }
                }
            }
        }

        let has_canvas_behavior = spec.specification.behaviors.iter().any(|b| {
            b.id.to_lowercase().contains("canvas")
                || b.description.to_lowercase().contains("canvas")
                || b.then.iter().any(|t| t.to_lowercase().contains("canvas"))
        });

        if has_canvas_behavior {
            let has_visual_feedback = spec.specification.behaviors.iter().any(|b| {
                b.then.iter().any(|t| {
                    t.to_lowercase().contains("display")
                        || t.to_lowercase().contains("show")
                        || t.to_lowercase().contains("render")
                        || t.to_lowercase().contains("visual")
                        || t.to_lowercase().contains("ui")
                        || t.to_lowercase().contains("feedback")
                })
            });

            if !has_visual_feedback {
                let issue = LintIssue {
                    rule_id: "SPEC-040".to_string(),
                    rule_name: "canvas-behavior-requires-visual-feedback".to_string(),
                    severity: spec_040_severity.clone(),
                    message: "Canvas behaviors should specify visual feedback for user experience"
                        .to_string(),
                    line: None,
                };
                if issue.severity == "error" {
                    report.errors.push(issue);
                } else {
                    report.warnings.push(issue);
                }
            }
        }

        report.categories.insert(
            "Security".to_string(),
            CategoryScore {
                score: 100,
                details: "Enumeration prevention and rate limiting checked".to_string(),
            },
        );
    }

    fn check_testability(rules: &LintRules, spec: &Spec, report: &mut LintReport) {
        let spec_030_rule = rules.rules.iter().find(|r| r.id == "SPEC-030");
        let severity = spec_030_rule
            .map(|r| r.severity.clone())
            .unwrap_or_else(|| "error".to_string());

        let observable_terms = ["http", "response", "status", "body", "api", "event"];
        let non_observable_count = spec
            .specification
            .behaviors
            .iter()
            .flat_map(|behavior| behavior.then.iter())
            .filter(|then_clause| {
                let is_observable = observable_terms
                    .iter()
                    .any(|term| then_clause.to_lowercase().contains(term));
                !is_observable && !then_clause.contains("audit")
            })
            .count();

        let score = if non_observable_count > 0 { 90 } else { 100 };
        if non_observable_count > 0 {
            let issue = LintIssue {
                rule_id: "SPEC-030".to_string(),
                rule_name: "behaviors-are-observable".to_string(),
                severity: severity.clone(),
                message: format!(
                    "{} behaviors may not have observable outcomes",
                    non_observable_count
                ),
                line: None,
            };
            if issue.severity == "error" {
                report.errors.push(issue);
            } else {
                report.warnings.push(issue);
            }
        }

        report.categories.insert(
            "Testability".to_string(),
            CategoryScore {
                score,
                details: format!("{} behaviors checked", spec.specification.behaviors.len()),
            },
        );
    }

    fn check_data_model(rules: &LintRules, spec: &Spec, report: &mut LintReport) {
        let spec_002_rule = rules.rules.iter().find(|r| r.id == "SPEC-002");
        let severity = spec_002_rule
            .map(|r| r.severity.clone())
            .unwrap_or_else(|| "error".to_string());

        let mut score = 100;

        if let Some(data_model) = &spec.specification.data_model {
            if let Some(transitions) = &data_model.state_transitions {
                if !transitions.is_empty() && spec.specification.context.invariants.is_empty() {
                    score = 88;
                    let issue = LintIssue {
                        rule_id: "SPEC-002".to_string(),
                        rule_name: "every-state-transition-has-invariant-check".to_string(),
                        severity: severity.clone(),
                        message: "State transitions found but no invariants defined".to_string(),
                        line: None,
                    };
                    if issue.severity == "error" {
                        report.errors.push(issue);
                    } else {
                        report.warnings.push(issue);
                    }
                }
            }
        }

        report.categories.insert(
            "Data Model".to_string(),
            CategoryScore {
                score,
                details: "Data model consistency checked".to_string(),
            },
        );
    }
}
