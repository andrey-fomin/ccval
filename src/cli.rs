pub const HELP_TEXT: &str = "Usage: ccval [-c <path>] [-r <path>] [-T] [-- <git-log-args>...]
       ccval [-c <path>] --stdin
       ccval [-c <path>] -f <path>
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
    pub repository_path: Option<String>,
    pub trust_repo: bool,
    pub input_mode: InputMode,
}

#[derive(Debug, PartialEq)]
pub enum CliAction {
    Run(CliOptions),
    ShowHelp,
}

pub fn parse_args<I>(args: I) -> Result<CliAction, String>
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

    let mut config_path = None;
    let mut repository_path = None;
    let mut file_path = None;
    let mut stdin_mode = false;
    let mut show_help = false;
    let mut trust_repo = false;
    let mut args = before_separator.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => show_help = true,
            "--trust-repo" | "-T" => {
                if trust_repo {
                    return Err(format!(
                        "--trust-repo/-T may be specified only once. {}",
                        HELP_HINT
                    ));
                }
                trust_repo = true;
            }
            "--stdin" => {
                if stdin_mode {
                    return Err(format!("--stdin may be specified only once. {}", HELP_HINT));
                }
                stdin_mode = true;
            }
            "--config" | "-c" => {
                let Some(path) = args.next() else {
                    return Err(format!("missing value for {}. {}", arg, HELP_HINT));
                };
                if config_path.is_some() {
                    return Err(format!(
                        "--config/-c may be specified only once. {}",
                        HELP_HINT
                    ));
                }
                config_path = Some(path);
            }
            "--repository" | "-r" => {
                let Some(path) = args.next() else {
                    return Err(format!("missing value for {}. {}", arg, HELP_HINT));
                };
                if repository_path.is_some() {
                    return Err(format!(
                        "--repository/-r may be specified only once. {}",
                        HELP_HINT
                    ));
                }
                repository_path = Some(path);
            }
            "--file" | "-f" => {
                let Some(path) = args.next() else {
                    return Err(format!("missing value for {}. {}", arg, HELP_HINT));
                };
                if file_path.is_some() {
                    return Err(format!(
                        "--file/-f may be specified only once. {}",
                        HELP_HINT
                    ));
                }
                file_path = Some(path);
            }
            _ => return Err(format!("unknown argument '{}'. {}", arg, HELP_HINT)),
        }
    }

    if show_help {
        if config_path.is_some()
            || repository_path.is_some()
            || file_path.is_some()
            || stdin_mode
            || trust_repo
            || seen_separator
        {
            return Err(format!(
                "--help/-h must be used without other arguments. {}",
                HELP_HINT
            ));
        }
        return Ok(CliAction::ShowHelp);
    }

    if stdin_mode && file_path.is_some() {
        return Err(format!(
            "--stdin cannot be combined with --file/-f. {}",
            HELP_HINT
        ));
    }

    if stdin_mode && repository_path.is_some() {
        return Err(format!(
            "--repository/-r cannot be used with --stdin. {}",
            HELP_HINT
        ));
    }

    if file_path.is_some() && repository_path.is_some() {
        return Err(format!(
            "--repository/-r cannot be used with --file/-f. {}",
            HELP_HINT
        ));
    }

    if trust_repo && stdin_mode {
        return Err(format!(
            "--trust-repo/-T cannot be used with --stdin. {}",
            HELP_HINT
        ));
    }

    if trust_repo && file_path.is_some() {
        return Err(format!(
            "--trust-repo/-T cannot be used with --file/-f. {}",
            HELP_HINT
        ));
    }

    if stdin_mode && seen_separator {
        return Err(format!(
            "--stdin cannot be combined with git arguments after --. {}",
            HELP_HINT
        ));
    }

    if file_path.is_some() && seen_separator {
        return Err(format!(
            "--file/-f cannot be combined with git arguments after --. {}",
            HELP_HINT
        ));
    }

    let input_mode = if seen_separator {
        if after_separator.is_empty() {
            return Err(format!(
                "expected at least one git argument after --. {}",
                HELP_HINT
            ));
        }
        InputMode::Git {
            git_args: after_separator,
        }
    } else if let Some(path) = file_path {
        InputMode::File { path }
    } else if stdin_mode {
        InputMode::Stdin
    } else {
        InputMode::Git {
            git_args: vec!["-1".to_string()],
        }
    };

    Ok(CliAction::Run(CliOptions {
        config_path,
        repository_path,
        trust_repo,
        input_mode,
    }))
}

#[cfg(test)]
mod tests {
    use super::{CliAction, CliOptions, InputMode, parse_args};

    fn parse_from(args: &[&str]) -> Result<CliAction, String> {
        parse_args(args.iter().map(|arg| (*arg).to_string()))
    }

    fn make_options(
        config_path: Option<String>,
        repository_path: Option<String>,
        input_mode: InputMode,
    ) -> CliOptions {
        CliOptions {
            config_path,
            repository_path,
            trust_repo: false,
            input_mode,
        }
    }

    fn make_options_with_trust(
        config_path: Option<String>,
        repository_path: Option<String>,
        trust_repo: bool,
        input_mode: InputMode,
    ) -> CliOptions {
        CliOptions {
            config_path,
            repository_path,
            trust_repo,
            input_mode,
        }
    }

    #[test]
    fn parse_config_path_long_flag() {
        let action = parse_from(&["--config", "custom.yaml"]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options(
                Some("custom.yaml".to_string()),
                None,
                InputMode::Git {
                    git_args: vec!["-1".to_string()],
                },
            ))
        );
    }

    #[test]
    fn parse_config_path_short_flag() {
        let action = parse_from(&["-c", "custom.yaml"]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options(
                Some("custom.yaml".to_string()),
                None,
                InputMode::Git {
                    git_args: vec!["-1".to_string()],
                },
            ))
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
        let action = parse_from(&[]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options(
                None,
                None,
                InputMode::Git {
                    git_args: vec!["-1".to_string()],
                },
            ))
        );
    }

    #[test]
    fn parse_stdin_mode() {
        let action = parse_from(&["--stdin"]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options(None, None, InputMode::Stdin,))
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
                InputMode::File {
                    path: "COMMIT_EDITMSG".to_string(),
                },
            ))
        );
    }

    #[test]
    fn parse_file_mode_short_flag() {
        let action = parse_from(&["-f", "COMMIT_EDITMSG"]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options(
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
    fn parse_repeated_config_is_rejected() {
        assert_eq!(
            parse_from(&["--config", "a.yaml", "-c", "b.yaml"]).unwrap_err(),
            "--config/-c may be specified only once. Run with --help or -h for usage information.",
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
    fn parse_repository_long_flag() {
        let action = parse_from(&["--repository", "/path/to/repo"]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options(
                None,
                Some("/path/to/repo".to_string()),
                InputMode::Git {
                    git_args: vec!["-1".to_string()],
                },
            ))
        );
    }

    #[test]
    fn parse_repository_short_flag() {
        let action = parse_from(&["-r", "/path/to/repo"]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options(
                None,
                Some("/path/to/repo".to_string()),
                InputMode::Git {
                    git_args: vec!["-1".to_string()],
                },
            ))
        );
    }

    #[test]
    fn parse_repository_with_git_args() {
        let action = parse_from(&["-r", "/path/to/repo", "--", "HEAD~5..HEAD"]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options(
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
    fn parse_trust_repo_long_flag() {
        let action = parse_from(&["--trust-repo"]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options_with_trust(
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
    fn parse_trust_repo_short_flag() {
        let action = parse_from(&["-T"]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options_with_trust(
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
    fn parse_trust_repo_with_repository() {
        let action = parse_from(&["-r", "/repo", "-T"]).unwrap();
        assert_eq!(
            action,
            CliAction::Run(make_options_with_trust(
                None,
                Some("/repo".to_string()),
                true,
                InputMode::Git {
                    git_args: vec!["-1".to_string()],
                },
            ))
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
                true,
                InputMode::Git {
                    git_args: vec!["HEAD~5..HEAD".to_string()],
                },
            ))
        );
    }
}
