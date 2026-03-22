use crate::config::{Config, FieldRules};
use crate::parser::Commit;

pub fn validate(commit: &Commit, config: &Config) -> Vec<String> {
    let mut errors = Vec::new();

    if let Some(rules) = &config.message {
        validate_field_rules(rules, &commit.message, "message", true, &mut errors);
    }
    if let Some(rules) = &config.header {
        validate_field_rules(rules, &commit.header, "header", true, &mut errors);
    }
    if let Some(rules) = &config.r#type {
        validate_field_rules(rules, &commit.r#type, "type", false, &mut errors);
    }
    if let Some(rules) = &config.scope {
        if let Some(scope) = commit.scope.as_deref() {
            validate_field_rules(rules, scope, "scope", false, &mut errors);
        } else if rules.required == Some(true) {
            errors.push("required scope is missing".to_string());
        }
    }
    if let Some(rules) = &config.description {
        validate_field_rules(
            rules,
            &commit.description,
            "description",
            false,
            &mut errors,
        );
    }
    if let Some(rules) = &config.body {
        if let Some(body) = commit.body.as_deref() {
            validate_field_rules(rules, body, "body", true, &mut errors);
        } else if rules.required == Some(true) {
            errors.push("required body is missing".to_string());
        }
    }

    if let Some(rules) = &config.footer_token {
        for footer in &commit.footers {
            validate_field_rules(rules, &footer.token, "footer-token", false, &mut errors);
        }
    }
    if let Some(rules) = &config.footer_value {
        for footer in &commit.footers {
            validate_field_rules(rules, &footer.value, "footer-value", true, &mut errors);
        }
    }

    if let Some(footers_rules) = &config.footers {
        for (footer_name, rules) in footers_rules {
            let mut found = false;
            for footer in &commit.footers {
                if footer.token == *footer_name {
                    found = true;
                    validate_field_rules(
                        rules,
                        &footer.value,
                        &format!("footer '{footer_name}'"),
                        true,
                        &mut errors,
                    );
                }
            }
            if !found && rules.required == Some(true) {
                errors.push(format!("required footer '{footer_name}' is missing"));
            }
        }
    }

    errors
}

fn validate_field_rules(
    rules: &FieldRules,
    text: &str,
    field_name: &str,
    ends_with_newline: bool,
    errors: &mut Vec<String>,
) {
    if rules.forbidden == Some(true) {
        errors.push(format!("{field_name} is forbidden"));
        return;
    }

    if let Some(max_len) = rules.max_length
        && text.len() > max_len
    {
        errors.push(format!(
            "{} length {} is greater than {}",
            field_name,
            text.len(),
            max_len
        ));
    }

    if let Some(max_line_len) = rules.max_line_length {
        let lines: Vec<&str> = if ends_with_newline {
            // exclude the trailing newline from line length calculation
            text.strip_suffix('\n')
                .unwrap_or(text)
                .split('\n')
                .collect()
        } else {
            text.split('\n').collect()
        };
        for line in lines {
            if line.len() > max_line_len {
                errors.push(format!(
                    "{} line length {} is greater than {}",
                    field_name,
                    line.len(),
                    max_line_len
                ));
                break; // only report once per field
            }
        }
    }

    if let Some(regexes) = rules.regexes.as_ref() {
        for regex in regexes {
            if !regex.is_match(text) {
                errors.push(format!(
                    "{field_name} does not match regex '{}'",
                    regex.as_str()
                ));
            }
        }
    }

    if let Some(values) = &rules.values {
        let stripped_text = if ends_with_newline {
            text.strip_suffix('\n').unwrap_or(text)
        } else {
            text
        };
        if !values.contains(&stripped_text.to_string()) {
            errors.push(format!(
                "{field_name} '{stripped_text}' is not in allowed values: {values:?}"
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::validator::validate;

    fn parse_config(yaml: &str) -> Config {
        serde_yaml::from_str(yaml).unwrap()
    }

    #[test]
    fn test_validate_ok() {
        let commit = crate::parser::parse("feat: a good commit\n").unwrap();
        let config = parse_config("type:\n  values:\n    - feat\n    - fix\n");
        let errors = validate(&commit, &config);
        assert!(errors.is_empty(), "Expected no errors, got {errors:?}");
    }

    #[test]
    fn test_validate_invalid_type() {
        let commit = crate::parser::parse("foo: invalid type\n").unwrap();
        let config = parse_config("type:\n  values:\n    - feat\n    - fix\n");
        let errors = validate(&commit, &config);
        assert_eq!(
            errors,
            vec!["type 'foo' is not in allowed values: [\"feat\", \"fix\"]"]
        );
    }

    #[test]
    fn test_validate_max_line_length() {
        let commit = crate::parser::parse(
            "feat: this is a very long commit message description that goes over the limit\n",
        )
        .unwrap();
        let config = parse_config("message:\n  max-line-length: 50\n");
        let errors = validate(&commit, &config);
        assert_eq!(errors, vec!["message line length 77 is greater than 50"]);
    }

    #[test]
    fn test_validate_regexes_and_semantics() {
        let commit = crate::parser::parse("feat: subject\n\nbody line\n").unwrap();
        let config = parse_config(
            "body:\n  regexes:\n    - '(?s)^[^\\n].*'\n    - '(?s).*(?:[^\\n])\\n$'\n",
        );
        let errors = validate(&commit, &config);
        assert!(errors.is_empty(), "Expected no errors, got {errors:?}");
    }

    #[test]
    fn test_validate_description_rejects_surrounding_spaces() {
        let commit = crate::parser::parse("feat: subject \n").unwrap();
        let config = parse_config("description:\n  regexes:\n    - '^[^ ].*'\n    - '^.*[^ ]$'\n");
        let errors = validate(&commit, &config);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("description does not match regex"));
    }

    #[test]
    fn test_validate_multiple_errors() {
        let commit =
            crate::parser::parse("bar: x\n\nthis body is way too long for the limit\n").unwrap();
        let config = parse_config(
            "type:\n  values:\n    - feat\n    - fix\nmessage:\n  max-line-length: 20\n",
        );
        let errors = validate(&commit, &config);
        assert_eq!(
            errors,
            vec![
                "message line length 39 is greater than 20",
                "type \'bar\' is not in allowed values: [\"feat\", \"fix\"]"
            ]
        );
    }
}
