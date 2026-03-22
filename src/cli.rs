pub const HELP_TEXT: &str =
    "Usage: ccval [-c <path>] [-p <preset>] [-r <path>] [-T] [-- <git-log-args>...]
       ccval [-c <path>] [-p <preset>] --stdin
       ccval [-c <path>] [-p <preset>] -f <path>
       ccval -h

Validates commit messages from stdin, a file, or Git.

Modes:
  (default)            Validate commit(s) from git log
                       Use -- <git-log-args>... to pass arguments to git log
                       Default: -1 (last commit)

  --stdin              Read commit message from stdin
  -f, --file <path>    Read commit message from a file
  -h, --help           Show this help message

Options:
  -c, --config <path>  Use a custom config file path
  -p, --preset <name>  Use a built-in preset (default or strict)
  -r, --repository <path>
                       Path to Git repository working tree
                       Cannot be used with --stdin or --file
  -T, --trust-repo     Trust the repository despite ownership mismatch
                       Useful when running in containers or accessing
                       repositories owned by other users
                       Requires git mode (cannot use with --stdin or --file)

Examples:
  ccval                              # validate last commit
  ccval -- origin/main..HEAD         # validate commits on branch
  ccval -p strict                    # validate last commit with strict preset
  ccval -r /path/to/repo             # validate last commit in specific repo
  ccval -T                           # validate last commit, trusting repo
  ccval -r /repo -T                  # validate in container
  printf 'feat: msg\\n' | ccval --stdin
  ccval --file .git/COMMIT_EDITMSG
  ccval -c config.yaml --stdin
";

const HELP_HINT: &str = "Run with --help or -h for usage information.";

#[derive(Debug, PartialEq)]
pub enum InputMode {
    Stdin,
    File { path: String },
    Git { git_args: Vec<String> },
}

#[derive(Debug, PartialEq)]
pub struct CliOptions {
    pub config_path: Option<String>,
    pub preset: Option<String>,
    pub repository_path: Option<String>,
    pub trust_repo: bool,
    pub input_mode: InputMode,
}

#[derive(Debug, PartialEq)]
pub enum CliAction {
    Run(CliOptions),
    ShowHelp,
}

struct SplitArgs {
    before: Vec<String>,
    after: Vec<String>,
    saw_separator: bool,
}

#[derive(Default)]
struct ParsedFlags {
    config_path: Option<String>,
    preset: Option<String>,
    repository_path: Option<String>,
    file_path: Option<String>,
    stdin_mode: bool,
    show_help: bool,
    trust_repo: bool,
}

pub fn parse_args<I>(args: I) -> Result<CliAction, String>
where
    I: Iterator<Item = String>,
{
    let split_args = split_args(args);
    let parsed_flags = parse_flag_options(split_args.before)?;

    validate_help_usage(&parsed_flags, split_args.saw_separator)?;
    if parsed_flags.show_help {
        return Ok(CliAction::ShowHelp);
    }

    validate_option_combinations(&parsed_flags, split_args.saw_separator)?;
    let input_mode = build_input_mode(&parsed_flags, split_args.after, split_args.saw_separator)?;

    Ok(CliAction::Run(CliOptions {
        config_path: parsed_flags.config_path,
        preset: parsed_flags.preset,
        repository_path: parsed_flags.repository_path,
        trust_repo: parsed_flags.trust_repo,
        input_mode,
    }))
}

fn split_args<I>(args: I) -> SplitArgs
where
    I: Iterator<Item = String>,
{
    let mut before_separator = Vec::new();
    let mut after_separator = Vec::new();
    let mut seen_separator = false;

    for arg in args {
        if seen_separator {
            after_separator.push(arg);
        } else if arg == "--" {
            seen_separator = true;
        } else {
            before_separator.push(arg);
        }
    }

    SplitArgs {
        before: before_separator,
        after: after_separator,
        saw_separator: seen_separator,
    }
}

fn parse_flag_options(args: Vec<String>) -> Result<ParsedFlags, String> {
    let mut parsed_flags = ParsedFlags::default();
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => parsed_flags.show_help = true,
            "--trust-repo" | "-T" => {
                ensure_flag_not_set(parsed_flags.trust_repo, "--trust-repo/-T")?;
                parsed_flags.trust_repo = true;
            }
            "--stdin" => {
                ensure_flag_not_set(parsed_flags.stdin_mode, "--stdin")?;
                parsed_flags.stdin_mode = true;
            }
            "--config" | "-c" => {
                let path = next_arg_value(&mut args, &arg)?;
                ensure_option_not_set(parsed_flags.config_path.as_ref(), "--config/-c")?;
                parsed_flags.config_path = Some(path);
            }
            "--preset" | "-p" => {
                let preset = next_arg_value(&mut args, &arg)?;
                ensure_option_not_set(parsed_flags.preset.as_ref(), "--preset/-p")?;
                parsed_flags.preset = Some(preset);
            }
            "--repository" | "-r" => {
                let path = next_arg_value(&mut args, &arg)?;
                ensure_option_not_set(parsed_flags.repository_path.as_ref(), "--repository/-r")?;
                parsed_flags.repository_path = Some(path);
            }
            "--file" | "-f" => {
                let path = next_arg_value(&mut args, &arg)?;
                ensure_option_not_set(parsed_flags.file_path.as_ref(), "--file/-f")?;
                parsed_flags.file_path = Some(path);
            }
            _ => return Err(format!("unknown argument '{arg}'. {HELP_HINT}")),
        }
    }

    Ok(parsed_flags)
}

fn validate_help_usage(parsed_flags: &ParsedFlags, seen_separator: bool) -> Result<(), String> {
    if parsed_flags.show_help
        && (parsed_flags.config_path.is_some()
            || parsed_flags.preset.is_some()
            || parsed_flags.repository_path.is_some()
            || parsed_flags.file_path.is_some()
            || parsed_flags.stdin_mode
            || parsed_flags.trust_repo
            || seen_separator)
    {
        return Err(format!(
            "--help/-h must be used without other arguments. {HELP_HINT}"
        ));
    }

    Ok(())
}

fn validate_option_combinations(
    parsed_flags: &ParsedFlags,
    seen_separator: bool,
) -> Result<(), String> {
    if parsed_flags.stdin_mode && parsed_flags.file_path.is_some() {
        return Err(format!(
            "--stdin cannot be combined with --file/-f. {HELP_HINT}"
        ));
    }

    if parsed_flags.stdin_mode && parsed_flags.repository_path.is_some() {
        return Err(format!(
            "--repository/-r cannot be used with --stdin. {HELP_HINT}"
        ));
    }

    if parsed_flags.file_path.is_some() && parsed_flags.repository_path.is_some() {
        return Err(format!(
            "--repository/-r cannot be used with --file/-f. {HELP_HINT}"
        ));
    }

    if parsed_flags.trust_repo && parsed_flags.stdin_mode {
        return Err(format!(
            "--trust-repo/-T cannot be used with --stdin. {HELP_HINT}"
        ));
    }

    if parsed_flags.trust_repo && parsed_flags.file_path.is_some() {
        return Err(format!(
            "--trust-repo/-T cannot be used with --file/-f. {HELP_HINT}"
        ));
    }

    if parsed_flags.stdin_mode && seen_separator {
        return Err(format!(
            "--stdin cannot be combined with git arguments after --. {HELP_HINT}"
        ));
    }

    if parsed_flags.file_path.is_some() && seen_separator {
        return Err(format!(
            "--file/-f cannot be combined with git arguments after --. {HELP_HINT}"
        ));
    }

    Ok(())
}

fn build_input_mode(
    parsed_flags: &ParsedFlags,
    after_separator: Vec<String>,
    seen_separator: bool,
) -> Result<InputMode, String> {
    if seen_separator {
        if after_separator.is_empty() {
            return Err(format!(
                "expected at least one git argument after --. {HELP_HINT}"
            ));
        }

        return Ok(InputMode::Git {
            git_args: after_separator,
        });
    }

    if let Some(path) = &parsed_flags.file_path {
        return Ok(InputMode::File { path: path.clone() });
    }

    if parsed_flags.stdin_mode {
        return Ok(InputMode::Stdin);
    }

    Ok(InputMode::Git {
        git_args: vec!["-1".to_owned()],
    })
}

fn next_arg_value<I>(args: &mut I, arg: &str) -> Result<String, String>
where
    I: Iterator<Item = String>,
{
    args.next()
        .ok_or_else(|| format!("missing value for {arg}. {HELP_HINT}"))
}

fn ensure_flag_not_set(is_set: bool, option_name: &str) -> Result<(), String> {
    if is_set {
        return Err(format!(
            "{option_name} may be specified only once. {HELP_HINT}"
        ));
    }

    Ok(())
}

fn ensure_option_not_set<T>(value: Option<&T>, option_name: &str) -> Result<(), String> {
    if value.is_some() {
        return Err(format!(
            "{option_name} may be specified only once. {HELP_HINT}"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CliAction, CliOptions, InputMode, parse_args};

    fn parse_from(args: &[&str]) -> Result<CliAction, String> {
        parse_args(args.iter().map(|arg| (*arg).to_string()))
    }

    fn assert_run(action: CliAction, expected: CliOptions) {
        assert_eq!(action, CliAction::Run(expected));
    }

    fn assert_git_mode(action: CliAction, expected_args: Vec<&str>, trust_repo: bool) {
        assert_eq!(
            action,
            CliAction::Run(CliOptions {
                config_path: None,
                preset: None,
                repository_path: None,
                trust_repo,
                input_mode: InputMode::Git {
                    git_args: expected_args
                        .into_iter()
                        .map(|arg| arg.to_string())
                        .collect(),
                },
            })
        );
    }

    fn make_options(
        config_path: Option<String>,
        preset: Option<String>,
        repository_path: Option<String>,
        input_mode: InputMode,
    ) -> CliOptions {
        CliOptions {
            config_path,
            preset,
            repository_path,
            trust_repo: false,
            input_mode,
        }
    }

    fn make_options_with_trust(
        config_path: Option<String>,
        preset: Option<String>,
        repository_path: Option<String>,
        trust_repo: bool,
        input_mode: InputMode,
    ) -> CliOptions {
        CliOptions {
            config_path,
            preset,
            repository_path,
            trust_repo,
            input_mode,
        }
    }

    #[test]
    fn parse_config_path_matches_short_and_long_flags() {
        for args in [["--config", "custom.yaml"], ["-c", "custom.yaml"]] {
            let action = parse_from(&args).unwrap();
            assert_run(
                action,
                make_options(
                    Some("custom.yaml".to_string()),
                    None,
                    None,
                    InputMode::Git {
                        git_args: vec!["-1".to_string()],
                    },
                ),
            );
        }
    }

    #[test]
    fn accepts_alias_parity() {
        for (args, expected) in [
            (
                ["--preset", "strict"],
                make_options(
                    None,
                    Some("strict".to_string()),
                    None,
                    InputMode::Git {
                        git_args: vec!["-1".to_string()],
                    },
                ),
            ),
            (
                ["-p", "default"],
                make_options(
                    None,
                    Some("default".to_string()),
                    None,
                    InputMode::Git {
                        git_args: vec!["-1".to_string()],
                    },
                ),
            ),
            (
                ["--file", "COMMIT_EDITMSG"],
                make_options(
                    None,
                    None,
                    None,
                    InputMode::File {
                        path: "COMMIT_EDITMSG".to_string(),
                    },
                ),
            ),
            (
                ["-f", "COMMIT_EDITMSG"],
                make_options(
                    None,
                    None,
                    None,
                    InputMode::File {
                        path: "COMMIT_EDITMSG".to_string(),
                    },
                ),
            ),
            (
                ["--repository", "/path/to/repo"],
                make_options(
                    None,
                    None,
                    Some("/path/to/repo".to_string()),
                    InputMode::Git {
                        git_args: vec!["-1".to_string()],
                    },
                ),
            ),
            (
                ["-r", "/path/to/repo"],
                make_options(
                    None,
                    None,
                    Some("/path/to/repo".to_string()),
                    InputMode::Git {
                        git_args: vec!["-1".to_string()],
                    },
                ),
            ),
        ] {
            assert_run(parse_from(&args).unwrap(), expected);
        }

        assert_eq!(parse_from(&["-h"]).unwrap(), CliAction::ShowHelp);
        assert_eq!(
            parse_from(&["-T"]).unwrap(),
            CliAction::Run(make_options_with_trust(
                None,
                None,
                None,
                true,
                InputMode::Git {
                    git_args: vec!["-1".to_string()]
                }
            ))
        );
        assert_eq!(parse_from(&["--help"]).unwrap(), CliAction::ShowHelp);
    }

    #[test]
    fn rejects_help_with_file_or_repository() {
        for args in [
            ["--help", "--file", "msg.txt"],
            ["--help", "--repository", "/repo"],
        ] {
            assert_eq!(
                parse_from(&args).unwrap_err(),
                "--help/-h must be used without other arguments. Run with --help or -h for usage information."
            );
        }
    }

    #[test]
    fn accepts_separator_with_flags_before_it() {
        let action = parse_from(&["-c", "custom.yaml", "-r", "/repo", "-T", "--", "HEAD"]).unwrap();
        assert_run(
            action,
            make_options_with_trust(
                Some("custom.yaml".to_string()),
                None,
                Some("/repo".to_string()),
                true,
                InputMode::Git {
                    git_args: vec!["HEAD".to_string()],
                },
            ),
        );
    }

    #[test]
    fn parse_config_path_missing_value() {
        assert_eq!(
            parse_from(&["--config"]).unwrap_err(),
            "missing value for --config. Run with --help or -h for usage information."
        );
    }

    #[test]
    fn parse_unknown_arg() {
        assert_eq!(
            parse_from(&["--unknown"]).unwrap_err(),
            "unknown argument '--unknown'. Run with --help or -h for usage information."
        );
    }

    #[test]
    fn parse_default_git_mode() {
        assert_git_mode(parse_from(&[]).unwrap(), vec!["-1"], false);
    }

    #[test]
    fn parse_stdin_mode() {
        let action = parse_from(&["--stdin"]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options(None, None, None, InputMode::Stdin,))
        );
    }

    #[test]
    fn parse_stdin_with_config() {
        let action = parse_from(&["-c", "custom.yaml", "--stdin"]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options(
                Some("custom.yaml".to_string()),
                None,
                None,
                InputMode::Stdin,
            ))
        );
    }

    #[test]
    fn parse_stdin_repeated_is_rejected() {
        assert_eq!(
            parse_from(&["--stdin", "--stdin"]).unwrap_err(),
            "--stdin may be specified only once. Run with --help or -h for usage information.",
        );
    }

    #[test]
    fn parse_stdin_with_file_is_rejected() {
        assert_eq!(
            parse_from(&["--stdin", "--file", "msg.txt"]).unwrap_err(),
            "--stdin cannot be combined with --file/-f. Run with --help or -h for usage information.",
        );
    }

    #[test]
    fn parse_trust_repo_with_repository_is_accepted() {
        let action = parse_from(&["-r", "/repo", "-T"]).unwrap();
        assert_run(
            action,
            make_options_with_trust(
                None,
                None,
                Some("/repo".to_string()),
                true,
                InputMode::Git {
                    git_args: vec!["-1".to_string()],
                },
            ),
        );
    }

    #[test]
    fn parse_stdin_with_git_args_is_rejected() {
        assert_eq!(
            parse_from(&["--stdin", "--", "HEAD"]).unwrap_err(),
            "--stdin cannot be combined with git arguments after --. Run with --help or -h for usage information.",
        );
    }

    #[test]
    fn parse_help_with_stdin_is_rejected() {
        assert_eq!(
            parse_from(&["--help", "--stdin"]).unwrap_err(),
            "--help/-h must be used without other arguments. Run with --help or -h for usage information.",
        );
    }

    #[test]
    fn parse_help_short_flag() {
        assert_eq!(parse_from(&["-h"]).unwrap(), CliAction::ShowHelp);
    }

    #[test]
    fn parse_help_with_config_is_rejected() {
        assert_eq!(
            parse_from(&["--help", "--config", "custom.yaml"]).unwrap_err(),
            "--help/-h must be used without other arguments. Run with --help or -h for usage information.",
        );
    }

    #[test]
    fn parse_help_with_trust_repo_is_rejected() {
        assert_eq!(
            parse_from(&["-h", "-T"]).unwrap_err(),
            "--help/-h must be used without other arguments. Run with --help or -h for usage information.",
        );
    }

    #[test]
    fn parse_file_mode_long_flag() {
        let action = parse_from(&["--file", "COMMIT_EDITMSG"]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options(
                None,
                None,
                None,
                InputMode::File {
                    path: "COMMIT_EDITMSG".to_string(),
                },
            ))
        );
    }

    #[test]
    fn parse_file_missing_value() {
        assert_eq!(
            parse_from(&["--file"]).unwrap_err(),
            "missing value for --file. Run with --help or -h for usage information."
        );
    }

    #[test]
    fn rejects_short_missing_values() {
        for (args, flag) in [(["-p"], "-p"), (["-r"], "-r"), (["-f"], "-f")] {
            assert_eq!(
                parse_from(&args).unwrap_err(),
                format!("missing value for {flag}. Run with --help or -h for usage information.")
            );
        }
    }

    #[test]
    fn rejects_combination_orders() {
        for args in [
            &["--stdin", "--file", "msg.txt"][..],
            &["--file", "msg.txt", "--stdin"][..],
            &["-T", "--stdin"][..],
            &["--stdin", "-T"][..],
            &["-T", "-f", "msg.txt"][..],
            &["-f", "msg.txt", "-T"][..],
        ] {
            assert!(parse_from(args).is_err());
        }
    }

    #[test]
    fn parse_repeated_config_is_rejected() {
        assert_eq!(
            parse_from(&["--config", "a.yaml", "-c", "b.yaml"]).unwrap_err(),
            "--config/-c may be specified only once. Run with --help or -h for usage information.",
        );
    }

    #[test]
    fn parse_preset_missing_value() {
        assert_eq!(
            parse_from(&["--preset"]).unwrap_err(),
            "missing value for --preset. Run with --help or -h for usage information."
        );
    }

    #[test]
    fn parse_repeated_preset_is_rejected() {
        assert_eq!(
            parse_from(&["--preset", "default", "-p", "strict"]).unwrap_err(),
            "--preset/-p may be specified only once. Run with --help or -h for usage information.",
        );
    }

    #[test]
    fn parse_repeated_file_is_rejected() {
        assert_eq!(
            parse_from(&["--file", "a.txt", "-f", "b.txt"]).unwrap_err(),
            "--file/-f may be specified only once. Run with --help or -h for usage information.",
        );
    }

    #[test]
    fn parse_git_mode() {
        let action = parse_from(&["--", "HEAD"]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options(
                None,
                None,
                None,
                InputMode::Git {
                    git_args: vec!["HEAD".to_string()],
                },
            ))
        );
    }

    #[test]
    fn parse_git_mode_with_multiple_args() {
        let action =
            parse_from(&["-c", "custom.yaml", "--", "master..HEAD", "--no-merges"]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options(
                Some("custom.yaml".to_string()),
                None,
                None,
                InputMode::Git {
                    git_args: vec!["master..HEAD".to_string(), "--no-merges".to_string()],
                },
            ))
        );
    }

    #[test]
    fn parse_separator_without_git_args_is_rejected() {
        assert_eq!(
            parse_from(&["--"]).unwrap_err(),
            "expected at least one git argument after --. Run with --help or -h for usage information.",
        );
    }

    #[test]
    fn parse_help_with_git_args_is_rejected() {
        assert_eq!(
            parse_from(&["--help", "--", "HEAD"]).unwrap_err(),
            "--help/-h must be used without other arguments. Run with --help or -h for usage information.",
        );
    }

    #[test]
    fn parse_file_with_git_args_is_rejected() {
        assert_eq!(
            parse_from(&["--file", "COMMIT_EDITMSG", "--", "HEAD"]).unwrap_err(),
            "--file/-f cannot be combined with git arguments after --. Run with --help or -h for usage information.",
        );
    }

    #[test]
    fn parse_repository_with_git_args() {
        let action = parse_from(&["-r", "/path/to/repo", "--", "HEAD~5..HEAD"]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options(
                None,
                None,
                Some("/path/to/repo".to_string()),
                InputMode::Git {
                    git_args: vec!["HEAD~5..HEAD".to_string()],
                },
            ))
        );
    }

    #[test]
    fn parse_repository_with_config() {
        let action = parse_from(&["-c", "config.yaml", "-r", "/repo"]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options(
                Some("config.yaml".to_string()),
                None,
                Some("/repo".to_string()),
                InputMode::Git {
                    git_args: vec!["-1".to_string()],
                },
            ))
        );
    }

    #[test]
    fn parse_repository_missing_value() {
        assert_eq!(
            parse_from(&["--repository"]).unwrap_err(),
            "missing value for --repository. Run with --help or -h for usage information."
        );
    }

    #[test]
    fn parse_repository_repeated_is_rejected() {
        assert_eq!(
            parse_from(&["-r", "a", "-r", "b"]).unwrap_err(),
            "--repository/-r may be specified only once. Run with --help or -h for usage information.",
        );
    }

    #[test]
    fn parse_repository_with_stdin_is_rejected() {
        assert_eq!(
            parse_from(&["-r", "/repo", "--stdin"]).unwrap_err(),
            "--repository/-r cannot be used with --stdin. Run with --help or -h for usage information.",
        );
    }

    #[test]
    fn parse_repository_with_file_is_rejected() {
        assert_eq!(
            parse_from(&["-r", "/repo", "--file", "msg.txt"]).unwrap_err(),
            "--repository/-r cannot be used with --file/-f. Run with --help or -h for usage information.",
        );
    }

    #[test]
    fn parse_preset_with_config() {
        let action = parse_from(&["-c", "config.yaml", "-p", "strict"]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options(
                Some("config.yaml".to_string()),
                Some("strict".to_string()),
                None,
                InputMode::Git {
                    git_args: vec!["-1".to_string()],
                },
            ))
        );
    }

    #[test]
    fn parse_help_with_preset_is_rejected() {
        assert_eq!(
            parse_from(&["--help", "--preset", "strict"]).unwrap_err(),
            "--help/-h must be used without other arguments. Run with --help or -h for usage information.",
        );
    }

    #[test]
    fn parse_trust_repo_with_config() {
        let action = parse_from(&["-c", "config.yaml", "-T"]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options_with_trust(
                Some("config.yaml".to_string()),
                None,
                None,
                true,
                InputMode::Git {
                    git_args: vec!["-1".to_string()],
                },
            ))
        );
    }

    #[test]
    fn parse_trust_repo_with_stdin_is_rejected() {
        assert_eq!(
            parse_from(&["--stdin", "-T"]).unwrap_err(),
            "--trust-repo/-T cannot be used with --stdin. Run with --help or -h for usage information.",
        );
    }

    #[test]
    fn parse_trust_repo_with_file_is_rejected() {
        assert_eq!(
            parse_from(&["-f", "msg.txt", "-T"]).unwrap_err(),
            "--trust-repo/-T cannot be used with --file/-f. Run with --help or -h for usage information.",
        );
    }

    #[test]
    fn parse_trust_repo_repeated_is_rejected() {
        assert_eq!(
            parse_from(&["-T", "-T"]).unwrap_err(),
            "--trust-repo/-T may be specified only once. Run with --help or -h for usage information.",
        );
    }

    #[test]
    fn parse_trust_repo_with_git_args() {
        let action = parse_from(&["-T", "--", "HEAD~5..HEAD"]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options_with_trust(
                None,
                None,
                None,
                true,
                InputMode::Git {
                    git_args: vec!["HEAD~5..HEAD".to_string()],
                },
            ))
        );
    }

    #[test]
    fn parse_separator_passes_flag_like_git_args() {
        let action = parse_from(&["--", "--stdin", "-T", "-h"]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options(
                None,
                None,
                None,
                InputMode::Git {
                    git_args: vec!["--stdin".to_string(), "-T".to_string(), "-h".to_string()],
                },
            ))
        );
    }
}
