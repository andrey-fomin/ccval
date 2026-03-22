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

    fn assert_ok(message: &str) -> Commit {
        parse(message).unwrap()
    }

    fn assert_err(message: &str) -> ParseError {
        parse(message).unwrap_err()
    }

    #[test]
    fn ok_header_minimal() {
        let commit = assert_ok("type1: description text\n");
        assert_eq!(commit.r#type, "type1");
        assert_eq!(commit.scope, None);
        assert!(!commit.breaking);
        assert_eq!(commit.description, "description text");
        assert_eq!(commit.body, None);
        assert!(commit.footers.is_empty());
    }

    #[test]
    fn ok_header_with_scope() {
        let commit = assert_ok("type1(scope1): description text\n");
        assert_eq!(commit.r#type, "type1");
        assert_eq!(commit.scope, Some("scope1".to_string()));
        assert!(!commit.breaking);
        assert_eq!(commit.description, "description text");
    }

    #[test]
    fn ok_header_with_breaking() {
        let commit = assert_ok("type1!: description text\n");
        assert_eq!(commit.r#type, "type1");
        assert_eq!(commit.scope, None);
        assert!(commit.breaking);
        assert_eq!(commit.description, "description text");
    }

    #[test]
    fn ok_header_preserves_description_surrounding_spaces() {
        let commit = assert_ok("type1(scope1):  description \n");
        assert_eq!(commit.r#type, "type1");
        assert_eq!(commit.scope, Some("scope1".to_string()));
        assert_eq!(commit.description, " description ");
    }

    #[test]
    fn ok_body_basic() {
        let commit = assert_ok("type1: description text\n\nbody line 1\nbody line 2\n");
        assert_eq!(commit.body, Some("body line 1\nbody line 2\n".to_string()));
    }

    #[test]
    fn ok_footers_multiple() {
        let commit = assert_ok(
            "type1: description text\n\nfooter-token1: footer value 1\nfooter-token2: footer value 2\n",
        );
        assert_eq!(commit.footers.len(), 2);
        assert_eq!(commit.footers[0].token, "footer-token1");
        assert_eq!(commit.footers[0].value, "footer value 1\n");
        assert_eq!(commit.footers[1].token, "footer-token2");
        assert_eq!(commit.footers[1].value, "footer value 2\n");
    }

    #[test]
    fn ok_full_commit() {
        let commit = assert_ok(
            "type1(scope1)!: description text\n\nbody line 1\nbody line 2\n\nfooter-token1: footer value 1\nfooter-token2: footer value 2\n",
        );
        assert_eq!(commit.r#type, "type1");
        assert_eq!(commit.scope, Some("scope1".to_string()));
        assert!(commit.breaking);
        assert_eq!(commit.description, "description text");
        assert_eq!(commit.body, Some("body line 1\nbody line 2\n".to_string()));
        assert_eq!(commit.footers.len(), 2);
        assert_eq!(commit.footers[0].token, "footer-token1");
        assert_eq!(commit.footers[0].value, "footer value 1\n");
        assert_eq!(commit.footers[1].token, "footer-token2");
        assert_eq!(commit.footers[1].value, "footer value 2\n");
    }

    #[test]
    fn ok_body_with_empty_lines() {
        let commit = assert_ok("type1: description text\n\nbody line 1\n\nbody line 2\n");
        assert_eq!(
            commit.body,
            Some("body line 1\n\nbody line 2\n".to_string())
        );
    }

    #[test]
    fn ok_footer_multiline_value() {
        let commit =
            assert_ok("type1: description text\n\nfooter-token1: footer line 1\nfooter line 2\n");
        assert_eq!(commit.footers.len(), 1);
        assert_eq!(commit.footers[0].token, "footer-token1");
        assert_eq!(commit.footers[0].value, "footer line 1\nfooter line 2\n");
    }

    #[test]
    fn ok_footer_breaking_change() {
        let commit =
            assert_ok("type1: description text\n\nBREAKING CHANGE: this is a breaking change\n");
        assert_eq!(commit.footers.len(), 1);
        assert_eq!(commit.footers[0].token, "BREAKING CHANGE");
        assert_eq!(commit.footers[0].value, "this is a breaking change\n");
    }

    #[test]
    fn ok_footer_with_hash_separator() {
        let commit = assert_ok("type1: description text\n\nCloses #123\n");
        assert_eq!(commit.footers.len(), 1);
        assert_eq!(commit.footers[0].token, "Closes");
        assert_eq!(commit.footers[0].value, "123\n");
    }

    #[test]
    fn ok_body_with_extra_blank_lines_before_footers() {
        let commit = assert_ok(
            "type1: description text\n\nbody line 1\n\n\nfooter-token1: footer value 1\n",
        );
        assert_eq!(commit.body, Some("body line 1\n\n".to_string()));
        assert_eq!(commit.footers.len(), 1);
        assert_eq!(commit.footers[0].token, "footer-token1");
    }

    #[test]
    fn ok_body_can_start_with_newline_and_footer_value_can_end_with_double_newline() {
        let commit = assert_ok(
            "type1(scope1): description text\n\n\nbody line 1\n\n\nfooter-token1: footer value 1\n\n",
        );
        assert_eq!(commit.body, Some("\nbody line 1\n\n".to_string()));
        assert_eq!(commit.footers.len(), 1);
        assert_eq!(commit.footers[0].token, "footer-token1");
        assert_eq!(commit.footers[0].value, "footer value 1\n\n");
    }

    #[test]
    fn ok_body_can_start_with_multiple_newlines() {
        let commit = assert_ok("type1: description text\n\n\n\nbody line 1\n");
        assert_eq!(commit.body, Some("\n\nbody line 1\n".to_string()));
        assert!(commit.footers.is_empty());
    }

    #[test]
    fn ok_footers_can_be_separated_by_multiple_empty_lines() {
        let commit = assert_ok(
            "type1: description text\n\nfooter-token1: footer value 1\n\nfooter-token2: footer value 2\n",
        );
        assert_eq!(commit.body, None);
        assert_eq!(commit.footers.len(), 2);
        assert_eq!(commit.footers[0].token, "footer-token1");
        assert_eq!(commit.footers[0].value, "footer value 1\n\n");
        assert_eq!(commit.footers[1].token, "footer-token2");
        assert_eq!(commit.footers[1].value, "footer value 2\n");
    }

    #[test]
    fn ok_footer_only_message_can_end_with_extra_empty_lines() {
        let commit = assert_ok("type1: description text\n\nCloses #123\n\n\n");
        assert_eq!(commit.body, None);
        assert_eq!(commit.footers.len(), 1);
        assert_eq!(commit.footers[0].token, "Closes");
        assert_eq!(commit.footers[0].value, "123\n\n\n");
    }

    #[test]
    fn ok_body_can_contain_footer_like_line_without_separator() {
        let commit = assert_ok("type1: description text\n\nbody line 1\nCloses #123\n");
        assert_eq!(commit.body, Some("body line 1\nCloses #123\n".to_string()));
        assert!(commit.footers.is_empty());
    }

    #[test]
    fn ok_footers_without_body() {
        let commit = assert_ok("type1: description text\n\nCloses #123\nReviewed-by: Jane\n");
        assert_eq!(commit.body, None);
        assert_eq!(commit.footers.len(), 2);
    }

    #[test]
    fn ok_footer_breaking_change_with_hash_separator() {
        let commit = assert_ok("type1: description text\n\nBREAKING CHANGE #123\n");
        assert_eq!(commit.footers.len(), 1);
        assert_eq!(commit.footers[0].token, "BREAKING CHANGE");
        assert_eq!(commit.footers[0].value, "123\n");
    }

    #[test]
    fn ko_header_without_trailing_newline() {
        let err = assert_err("type1: description text");
        assert_eq!(err, ParseError::NoNewlineAtEndOfHeader);
    }

    #[test]
    fn ok_crlf_line_endings_are_normalized() {
        let commit = assert_ok("type1: description text\r\n\r\nbody line 1\r\nCloses:value\r\n");
        assert_eq!(commit.header, "type1: description text\n");
        assert_eq!(commit.body, Some("body line 1\nCloses:value\n".to_string()));
        assert_eq!(
            commit.message,
            "type1: description text\n\nbody line 1\nCloses:value\n"
        );
    }

    #[test]
    fn ko_tab_is_rejected() {
        let err = assert_err("type1: description\ttext\n");
        assert_eq!(err, ParseError::NonPrintableCharacter('\t'));
    }

    #[test]
    fn ko_bare_carriage_return_is_rejected() {
        let err = assert_err("type1: description text\r");
        assert_eq!(err, ParseError::NonPrintableCharacter('\r'));
    }

    #[test]
    fn ko_nul_is_rejected() {
        let err = assert_err("type1: description\0text\n");
        assert_eq!(err, ParseError::NonPrintableCharacter('\0'));
    }

    #[test]
    fn ok_unicode_text_is_allowed() {
        let commit = assert_ok("föö(scöpé): décrïption text\n\nтело\n");
        assert_eq!(commit.r#type, "föö");
        assert_eq!(commit.scope, Some("scöpé".to_string()));
        assert_eq!(commit.description, "décrïption text");
        assert_eq!(commit.body, Some("тело\n".to_string()));
    }

    #[test]
    fn ko_body_without_trailing_newline() {
        let err = assert_err("type1: description text\n\nbody line 1\nbody line 2");
        assert_eq!(err, ParseError::NoNewlineAtEndOfBody);
    }

    #[test]
    fn ko_footer_without_trailing_newline() {
        let err = assert_err("type1: description text\n\nCloses #123");
        assert_eq!(err, ParseError::NoNewlineAtEndOfFooter);
    }

    #[test]
    fn ko_missing_type() {
        let err = assert_err(": description text\n");
        assert_eq!(err, ParseError::MissingType);
    }

    #[test]
    fn ko_missing_colon_and_space_after_type() {
        let err = assert_err("type1:\n");
        assert_eq!(err, ParseError::MissingColonAndSpace(5));
    }

    #[test]
    fn ko_empty_description() {
        let err = assert_err("type1: \n");
        assert_eq!(err, ParseError::MissingDescription(7));
    }

    #[test]
    fn ko_missing_colon_and_space_after_type_without_scope() {
        let err = assert_err("type1\n");
        assert_eq!(err, ParseError::MissingColonAndSpace(5));
    }

    #[test]
    fn ko_type_with_spaces() {
        let err = assert_err("my type: description text\n");
        assert_eq!(err, ParseError::MissingColonAndSpace(2));
    }

    #[test]
    fn ko_missing_colon_and_space_after_scope() {
        let err = assert_err("type1(scope1)\n");
        assert_eq!(err, ParseError::MissingColonAndSpace(13));
    }

    #[test]
    fn ko_unclosed_scope() {
        let err = assert_err("type1(scope1: description text\n");
        assert_eq!(err, ParseError::UnclosedScope(6));
    }

    #[test]
    fn ko_empty_scope() {
        let err = assert_err("type1(): description text\n");
        assert_eq!(err, ParseError::InvalidScope(6));
    }

    #[test]
    fn ko_scope_with_spaces() {
        let err = assert_err("type1(my scope): description text\n");
        assert_eq!(err, ParseError::InvalidScope(6));
    }

    #[test]
    fn ko_scope_ending_with_dash() {
        let err = assert_err("type1(scope-): description text\n");
        assert_eq!(err, ParseError::InvalidScope(6));
    }

    #[test]
    fn ko_missing_blank_line_before_body() {
        let err = assert_err("type1: description text\nbody line 1\n");
        assert_eq!(err, ParseError::MissingBlankLineAfterHeader);
    }

    #[test]
    fn ko_missing_blank_line_before_footer() {
        let err = assert_err("type1: description text\nCloses #123\n");
        assert_eq!(err, ParseError::MissingBlankLineAfterHeader);
    }

    #[test]
    fn ko_blank_line_without_body_or_footer() {
        let err = assert_err("type1: description text\n\n");
        assert_eq!(err, ParseError::MissingBodyOrFooterAfterBlankLine);
    }

    #[test]
    fn ok_footer_like_text_without_separator_stays_in_body() {
        let commit =
            assert_ok("type1: description text\n\nbody line 1\nCloses:value\nCloses#123\n");
        assert_eq!(
            commit.body,
            Some("body line 1\nCloses:value\nCloses#123\n".to_string())
        );
        assert!(commit.footers.is_empty());
    }

    #[test]
    fn ok_scope_allows_single_character_and_numbers() {
        let commit = assert_ok("type1(a1): description text\n");
        assert_eq!(commit.scope, Some("a1".to_string()));
    }
}
