#[derive(Debug, PartialEq)]
pub struct Commit {
    pub message: String,
    pub header: String,
    pub r#type: String,
    pub scope: Option<String>,
    pub breaking: bool,
    pub description: String,
    pub body: Option<String>,
    pub footers: Vec<Footer>,
}

#[derive(Debug, PartialEq)]
pub struct Footer {
    pub token: String,
    pub value: String,
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    NonPrintableCharacter(char),
    NoNewlineAtEndOfHeader,
    MissingType,
    InvalidScope(usize),
    UnclosedScope(usize),
    MissingColonAndSpace(usize),
    MissingDescription(usize),
    MissingBlankLineAfterHeader,
    MissingBodyOrFooterAfterBlankLine,
    NoNewlineAtEndOfBody,
    NoNewlineAtEndOfFooter,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::NonPrintableCharacter(ch) => write!(
                f,
                "Parsing error: Non-printable character U+{:04X} is not allowed",
                *ch as u32
            ),
            ParseError::NoNewlineAtEndOfHeader => {
                write!(f, "Parsing error: Header must end with a newline")
            }
            ParseError::MissingType => write!(f, "Parsing error at line 1:0: Missing commit type"),
            ParseError::InvalidScope(col) => {
                write!(f, "Parsing error at line 1:{col}: Invalid scope")
            }
            ParseError::UnclosedScope(col) => {
                write!(f, "Parsing error at line 1:{col}: Unclosed scope")
            }
            ParseError::MissingColonAndSpace(col) => write!(
                f,
                "Parsing error at line 1:{col}: Missing colon and space after type/scope"
            ),
            ParseError::MissingDescription(col) => {
                write!(f, "Parsing error at line 1:{col}: Missing description")
            }
            ParseError::MissingBlankLineAfterHeader => write!(
                f,
                "Parsing error: Body or footer must be separated from the header by a blank line"
            ),
            ParseError::MissingBodyOrFooterAfterBlankLine => {
                write!(f, "Parsing error: Expected body or footer after blank line")
            }
            ParseError::NoNewlineAtEndOfBody => {
                write!(f, "Parsing error: Body must end with a newline")
            }
            ParseError::NoNewlineAtEndOfFooter => {
                write!(f, "Parsing error: Footer must end with a newline")
            }
        }
    }
}

fn is_identifier_start(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn is_identifier_continue(c: char) -> bool {
    is_identifier_start(c) || c == '-'
}

fn parse_identifier_prefix(text: &str) -> Option<usize> {
    let mut chars = text.char_indices();
    let (_, first_char) = chars.next()?;
    if !is_identifier_start(first_char) {
        return None;
    }

    let mut end = first_char.len_utf8();
    for (idx, ch) in chars {
        if !is_identifier_continue(ch) {
            break;
        }
        end = idx + ch.len_utf8();
    }

    if text[..end].ends_with('-') {
        return None;
    }

    Some(end)
}

fn is_footer_start(line: &str) -> Option<(usize, usize)> {
    let line = line.strip_suffix('\n').unwrap_or(line);

    if line.starts_with("BREAKING CHANGE: ") {
        return Some((15, 17));
    }
    if line.starts_with("BREAKING CHANGE #") {
        return Some((15, 17));
    }

    let token_end = parse_identifier_prefix(line)?;
    if token_end + 2 > line.len() {
        return None;
    }

    let separator = &line[token_end..token_end + 2];
    if separator == ": " || separator == " #" {
        Some((token_end, token_end + 2))
    } else {
        None
    }
}

fn collect_lines(text: &str) -> Vec<(usize, &str)> {
    let mut offset = 0;
    let mut lines = Vec::new();
    for line in text.split_inclusive('\n') {
        lines.push((offset, line));
        offset += line.len();
    }
    if offset < text.len() {
        lines.push((offset, &text[offset..]));
    }
    lines
}

fn find_footer_start(lines: &[(usize, &str)]) -> Option<usize> {
    if lines
        .first()
        .is_some_and(|(_, line)| is_footer_start(line).is_some())
    {
        return Some(0);
    }

    for idx in 0..lines.len().saturating_sub(1) {
        if lines[idx].1 == "\n" && is_footer_start(lines[idx + 1].1).is_some() {
            return Some(idx + 1);
        }
    }

    None
}

fn normalize_newlines(message: &str) -> String {
    message.replace("\r\n", "\n")
}

fn validate_characters(message: &str) -> Result<(), ParseError> {
    for ch in message.chars() {
        if ch.is_control() && ch != '\n' {
            return Err(ParseError::NonPrintableCharacter(ch));
        }
    }
    Ok(())
}

fn parse_header(header: &str) -> Result<(&str, Option<&str>, bool, &str), ParseError> {
    let Some(type_end) = parse_identifier_prefix(header) else {
        return Err(ParseError::MissingType);
    };

    let commit_type = &header[..type_end];
    let mut current_idx = type_end;
    let mut scope = None;

    if current_idx < header.len() && header[current_idx..].starts_with('(') {
        let scope_start = current_idx + 1;
        let Some(scope_rel_end) = header[scope_start..].find(')') else {
            return Err(ParseError::UnclosedScope(scope_start));
        };
        let scope_end = scope_start + scope_rel_end;
        let scope_text = &header[scope_start..scope_end];
        let is_valid_scope = parse_identifier_prefix(scope_text)
            .is_some_and(|parsed_len| parsed_len == scope_text.len());
        if !is_valid_scope {
            return Err(ParseError::InvalidScope(scope_start));
        }
        scope = Some(scope_text);
        current_idx = scope_end + 1;
    }

    let breaking = if current_idx < header.len() && header[current_idx..].starts_with('!') {
        current_idx += 1;
        true
    } else {
        false
    };

    if current_idx >= header.len() || !header[current_idx..].starts_with(": ") {
        return Err(ParseError::MissingColonAndSpace(current_idx));
    }

    current_idx += 2;
    let description = &header[current_idx..];
    if description.is_empty() {
        return Err(ParseError::MissingDescription(current_idx));
    }

    Ok((commit_type, scope, breaking, description))
}

fn parse_footers(
    section: &str,
    lines: &[(usize, &str)],
    start_idx: usize,
) -> Result<Vec<Footer>, ParseError> {
    let mut footers = Vec::new();
    let mut idx = start_idx;

    while idx < lines.len() {
        let (token_end, value_start) = is_footer_start(lines[idx].1).unwrap();
        let token = &lines[idx].1[..token_end];
        let footer_value_offset = lines[idx].0 + value_start;

        let mut next_idx = idx + 1;
        while next_idx < lines.len() && is_footer_start(lines[next_idx].1).is_none() {
            next_idx += 1;
        }

        let footer_end = if next_idx < lines.len() {
            lines[next_idx].0
        } else {
            section.len()
        };
        let value = &section[footer_value_offset..footer_end];
        if !value.ends_with('\n') {
            return Err(ParseError::NoNewlineAtEndOfFooter);
        }

        footers.push(Footer {
            token: token.to_string(),
            value: value.to_string(),
        });
        idx = next_idx;
    }

    Ok(footers)
}

fn parse_message_section(rest: &str) -> Result<(Option<String>, Vec<Footer>), ParseError> {
    if rest.is_empty() {
        return Ok((None, Vec::new()));
    }

    if !rest.starts_with('\n') {
        return Err(ParseError::MissingBlankLineAfterHeader);
    }

    let section = &rest[1..];
    if section.is_empty() {
        return Err(ParseError::MissingBodyOrFooterAfterBlankLine);
    }

    let lines = collect_lines(section);
    let footer_start_idx = find_footer_start(&lines);

    let body_end = match footer_start_idx {
        Some(0) => 0,
        Some(idx) => lines[idx - 1].0,
        None => section.len(),
    };

    let body = if body_end > 0 {
        let body_str = &section[..body_end];
        if !body_str.ends_with('\n') {
            return Err(ParseError::NoNewlineAtEndOfBody);
        }
        Some(body_str.to_string())
    } else {
        None
    };

    let footers = match footer_start_idx {
        Some(start_idx) => parse_footers(section, &lines, start_idx)?,
        None => Vec::new(),
    };

    Ok((body, footers))
}

pub fn parse(message: &str) -> Result<Commit, ParseError> {
    let normalized_message = normalize_newlines(message);
    validate_characters(&normalized_message)?;

    if !normalized_message.contains('\n') {
        return Err(ParseError::NoNewlineAtEndOfHeader);
    }

    let (header, rest) = normalized_message.split_once('\n').unwrap();
    let (commit_type, scope, breaking, description) = parse_header(header)?;
    let (body, footers) = parse_message_section(rest)?;
    let header = format!("{header}\n");
    let commit_type = commit_type.to_string();
    let scope = scope.map(str::to_string);
    let description = description.to_string();

    Ok(Commit {
        message: normalized_message,
        header,
        r#type: commit_type,
        scope,
        breaking,
        description,
        body,
        footers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct ExpectedFooter {
        token: &'static str,
        value: &'static str,
    }

    struct ExpectedCommit {
        message: &'static str,
        header: &'static str,
        r#type: &'static str,
        scope: Option<&'static str>,
        breaking: bool,
        description: &'static str,
        body: Option<&'static str>,
        footers: Vec<ExpectedFooter>,
    }

    fn assert_ok_commit(name: &str, input: &str, expected: ExpectedCommit) {
        let commit = parse(input).unwrap();
        assert_eq!(commit.message, expected.message, "case: {name}");
        assert_eq!(commit.header, expected.header, "case: {name}");
        assert_eq!(commit.r#type, expected.r#type, "case: {name}");
        assert_eq!(commit.scope.as_deref(), expected.scope, "case: {name}");
        assert_eq!(commit.breaking, expected.breaking, "case: {name}");
        assert_eq!(commit.description, expected.description, "case: {name}");
        assert_eq!(commit.body.as_deref(), expected.body, "case: {name}");
        assert_eq!(
            commit
                .footers
                .iter()
                .map(|f| (f.token.as_str(), f.value.as_str()))
                .collect::<Vec<_>>(),
            expected
                .footers
                .iter()
                .map(|f| (f.token, f.value))
                .collect::<Vec<_>>(),
            "case: {name}"
        );
    }

    fn assert_err(name: &str, input: &str, expected: ParseError, expected_display: &str) {
        let err = parse(input).unwrap_err();
        assert_eq!(err, expected, "case: {name}");
        assert_eq!(err.to_string(), expected_display, "case: {name}");
    }

    #[test]
    fn valid_parse_cases() {
        for (name, input, expected) in [
            (
                "minimal header",
                "type1: description text\n",
                ExpectedCommit {
                    message: "type1: description text\n",
                    header: "type1: description text\n",
                    r#type: "type1",
                    scope: None,
                    breaking: false,
                    description: "description text",
                    body: None,
                    footers: vec![],
                },
            ),
            (
                "bare breaking header",
                "feat!: description text\n",
                ExpectedCommit {
                    message: "feat!: description text\n",
                    header: "feat!: description text\n",
                    r#type: "feat",
                    scope: None,
                    breaking: true,
                    description: "description text",
                    body: None,
                    footers: vec![],
                },
            ),
            (
                "breaking footer colon",
                "type1: description text\n\nBREAKING CHANGE: change log\n",
                ExpectedCommit {
                    message: "type1: description text\n\nBREAKING CHANGE: change log\n",
                    header: "type1: description text\n",
                    r#type: "type1",
                    scope: None,
                    breaking: false,
                    description: "description text",
                    body: None,
                    footers: vec![ExpectedFooter {
                        token: "BREAKING CHANGE",
                        value: "change log\n",
                    }],
                },
            ),
            (
                "breaking footer hash",
                "type1: description text\n\nBREAKING CHANGE #123\n",
                ExpectedCommit {
                    message: "type1: description text\n\nBREAKING CHANGE #123\n",
                    header: "type1: description text\n",
                    r#type: "type1",
                    scope: None,
                    breaking: false,
                    description: "description text",
                    body: None,
                    footers: vec![ExpectedFooter {
                        token: "BREAKING CHANGE",
                        value: "123\n",
                    }],
                },
            ),
            (
                "header with spaces preserved",
                "type1(scope1):  description \n",
                ExpectedCommit {
                    message: "type1(scope1):  description \n",
                    header: "type1(scope1):  description \n",
                    r#type: "type1",
                    scope: Some("scope1"),
                    breaking: false,
                    description: " description ",
                    body: None,
                    footers: vec![],
                },
            ),
            (
                "unicode text",
                "föö(scöpé): décrïption text\n\nтело\n",
                ExpectedCommit {
                    message: "föö(scöpé): décrïption text\n\nтело\n",
                    header: "föö(scöpé): décrïption text\n",
                    r#type: "föö",
                    scope: Some("scöpé"),
                    breaking: false,
                    description: "décrïption text",
                    body: Some("тело\n"),
                    footers: vec![],
                },
            ),
            (
                "full commit",
                "type1(scope1)!: description text\n\nbody line 1\nbody line 2\n\nfooter-token1: footer value 1\nfooter-token2: footer value 2\n",
                ExpectedCommit {
                    message: "type1(scope1)!: description text\n\nbody line 1\nbody line 2\n\nfooter-token1: footer value 1\nfooter-token2: footer value 2\n",
                    header: "type1(scope1)!: description text\n",
                    r#type: "type1",
                    scope: Some("scope1"),
                    breaking: true,
                    description: "description text",
                    body: Some("body line 1\nbody line 2\n"),
                    footers: vec![
                        ExpectedFooter {
                            token: "footer-token1",
                            value: "footer value 1\n",
                        },
                        ExpectedFooter {
                            token: "footer-token2",
                            value: "footer value 2\n",
                        },
                    ],
                },
            ),
            (
                "body contains footer-like line",
                "type1: description text\n\nbody line 1\nCloses #123\n",
                ExpectedCommit {
                    message: "type1: description text\n\nbody line 1\nCloses #123\n",
                    header: "type1: description text\n",
                    r#type: "type1",
                    scope: None,
                    breaking: false,
                    description: "description text",
                    body: Some("body line 1\nCloses #123\n"),
                    footers: vec![],
                },
            ),
            (
                "footer-only commit",
                "type1: description text\n\nCloses #123\nReviewed-by: Jane\n",
                ExpectedCommit {
                    message: "type1: description text\n\nCloses #123\nReviewed-by: Jane\n",
                    header: "type1: description text\n",
                    r#type: "type1",
                    scope: None,
                    breaking: false,
                    description: "description text",
                    body: None,
                    footers: vec![
                        ExpectedFooter {
                            token: "Closes",
                            value: "123\n",
                        },
                        ExpectedFooter {
                            token: "Reviewed-by",
                            value: "Jane\n",
                        },
                    ],
                },
            ),
            (
                "crlf with footer parsing",
                "type1: description text\r\n\r\nbody line 1\r\n\r\nBREAKING CHANGE: change\r\n",
                ExpectedCommit {
                    message: "type1: description text\n\nbody line 1\n\nBREAKING CHANGE: change\n",
                    header: "type1: description text\n",
                    r#type: "type1",
                    scope: None,
                    breaking: false,
                    description: "description text",
                    body: Some("body line 1\n"),
                    footers: vec![ExpectedFooter {
                        token: "BREAKING CHANGE",
                        value: "change\n",
                    }],
                },
            ),
            (
                "scope identifier breadth",
                "type1(a1): description text\n",
                ExpectedCommit {
                    message: "type1(a1): description text\n",
                    header: "type1(a1): description text\n",
                    r#type: "type1",
                    scope: Some("a1"),
                    breaking: false,
                    description: "description text",
                    body: None,
                    footers: vec![],
                },
            ),
        ] {
            assert_ok_commit(name, input, expected);
        }
    }

    #[test]
    fn invalid_parse_cases() {
        for (name, input, expected, display) in [
            (
                "missing type",
                ": description text\n",
                ParseError::MissingType,
                "Parsing error at line 1:0: Missing commit type",
            ),
            (
                "missing colon after type",
                "type1:\n",
                ParseError::MissingColonAndSpace(5),
                "Parsing error at line 1:5: Missing colon and space after type/scope",
            ),
            (
                "missing description",
                "type1: \n",
                ParseError::MissingDescription(7),
                "Parsing error at line 1:7: Missing description",
            ),
            (
                "unclosed scope",
                "type1(scope1: description text\n",
                ParseError::UnclosedScope(6),
                "Parsing error at line 1:6: Unclosed scope",
            ),
            (
                "invalid scope",
                "type1(my scope): description text\n",
                ParseError::InvalidScope(6),
                "Parsing error at line 1:6: Invalid scope",
            ),
            (
                "missing blank line before body",
                "type1: description text\nbody line 1\n",
                ParseError::MissingBlankLineAfterHeader,
                "Parsing error: Body or footer must be separated from the header by a blank line",
            ),
            (
                "blank line without body",
                "type1: description text\n\n",
                ParseError::MissingBodyOrFooterAfterBlankLine,
                "Parsing error: Expected body or footer after blank line",
            ),
            (
                "header newline",
                "type1: description text",
                ParseError::NoNewlineAtEndOfHeader,
                "Parsing error: Header must end with a newline",
            ),
            (
                "body newline",
                "type1: description text\n\nbody line 1\nbody line 2",
                ParseError::NoNewlineAtEndOfBody,
                "Parsing error: Body must end with a newline",
            ),
            (
                "footer newline",
                "type1: description text\n\nCloses #123",
                ParseError::NoNewlineAtEndOfFooter,
                "Parsing error: Footer must end with a newline",
            ),
            (
                "control char",
                "type1: description\ttext\n",
                ParseError::NonPrintableCharacter('\t'),
                "Parsing error: Non-printable character U+0009 is not allowed",
            ),
            (
                "carriage return",
                "type1: description\rtext\n",
                ParseError::NonPrintableCharacter('\r'),
                "Parsing error: Non-printable character U+000D is not allowed",
            ),
            (
                "nul byte",
                "type1: description\0text\n",
                ParseError::NonPrintableCharacter('\0'),
                "Parsing error: Non-printable character U+0000 is not allowed",
            ),
        ] {
            assert_err(name, input, expected, display);
        }
    }
}
