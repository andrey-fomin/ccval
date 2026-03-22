use crate::cli::{CliOptions, InputMode};
use crate::config::{Config, ConfigError};
use crate::git::{GitError, GitLoader};
use crate::{parser, validator};

#[derive(Debug)]
pub struct RunOutcome {
    pub parse_failures: usize,
    pub validation_failures: usize,
}

#[derive(Debug)]
pub enum AppError {
    Config(ConfigError),
    Git(GitError),
    StdinIo(std::io::Error),
    FileIo { path: String, error: std::io::Error },
}

pub fn run(options: CliOptions, git_loader: &dyn GitLoader) -> Result<RunOutcome, AppError> {
    let config = load_config(options.config_path.as_deref(), options.preset.as_deref())
        .map_err(AppError::Config)?;
    let inputs = load_inputs(
        options.input_mode,
        git_loader,
        options.repository_path.as_deref(),
        options.trust_repo,
    )?;
    let mut parse_failures = 0usize;
    let mut validation_failures = 0usize;

    for input in inputs {
        match parser::parse(&input.message) {
            Ok(commit) => {
                let errors = validator::validate(&commit, &config);
                if !errors.is_empty() {
                    validation_failures += 1;
                    if input.label != "stdin" {
                        eprintln!("{label}:", label = input.label);
                    }
                    for error in errors {
                        eprintln!("Validation error: {error}");
                    }
                }
            }
            Err(error) => {
                parse_failures += 1;
                if input.label != "stdin" {
                    eprintln!("{label}:", label = input.label);
                }
                eprintln!("{error}");
            }
        }
    }

    Ok(RunOutcome {
        parse_failures,
        validation_failures,
    })
}

#[derive(Debug)]
struct CommitInput {
    label: String,
    message: String,
}

fn load_config(config_path: Option<&str>, preset: Option<&str>) -> Result<Config, ConfigError> {
    Config::load_with_preset(config_path, preset)
}

fn load_inputs(
    input_mode: InputMode,
    git_loader: &dyn GitLoader,
    repository_path: Option<&str>,
    trust_repo: bool,
) -> Result<Vec<CommitInput>, AppError> {
    match input_mode {
        InputMode::Stdin => Ok(vec![CommitInput {
            label: "stdin".to_owned(),
            message: read_stdin().map_err(AppError::StdinIo)?,
        }]),
        InputMode::File { path } => {
            let message = std::fs::read_to_string(&path).map_err(|error| AppError::FileIo {
                path: path.clone(),
                error,
            })?;
            Ok(vec![CommitInput {
                label: path,
                message,
            }])
        }
        InputMode::Git { git_args } => Ok(git_loader
            .load_commits(&git_args, repository_path, trust_repo)
            .map_err(AppError::Git)?
            .into_iter()
            .map(|commit| CommitInput {
                label: format_commit_label(&commit.id, &commit.message),
                message: commit.message,
            })
            .collect()),
    }
}

fn format_commit_label(commit_id: &str, message: &str) -> String {
    let subject = message.lines().next().map(str::trim).unwrap_or_default();
    if subject.is_empty() {
        format!("commit {commit_id}")
    } else {
        format!("commit {commit_id}: {subject}")
    }
}

fn read_stdin() -> Result<String, std::io::Error> {
    use std::io::Read;

    let mut buffer = String::new();
    std::io::stdin().read_to_string(&mut buffer)?;
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::GitCommit;
    use std::cell::RefCell;
    use std::io::Write;
    use tempfile::NamedTempFile;

    type GitLoaderCall = (Vec<String>, Option<String>, bool);

    struct SpyGitLoader {
        seen_args: RefCell<Option<GitLoaderCall>>,
        commits: Vec<GitCommit>,
    }

    impl GitLoader for SpyGitLoader {
        fn load_commits(
            &self,
            args: &[String],
            repository_path: Option<&str>,
            trust_repo: bool,
        ) -> Result<Vec<GitCommit>, GitError> {
            self.seen_args.borrow_mut().replace((
                args.to_vec(),
                repository_path.map(str::to_string),
                trust_repo,
            ));
            Ok(self.commits.clone())
        }
    }

    struct MockGitLoader {
        commits: Vec<GitCommit>,
        error: Option<GitError>,
    }

    impl GitLoader for MockGitLoader {
        fn load_commits(
            &self,
            _args: &[String],
            _repository_path: Option<&str>,
            _trust_repo: bool,
        ) -> Result<Vec<GitCommit>, GitError> {
            if let Some(ref err) = self.error {
                return Err(match err {
                    GitError::GitFailed { code, stderr } => GitError::GitFailed {
                        code: *code,
                        stderr: stderr.clone(),
                    },
                    GitError::Io(error) => {
                        GitError::Io(std::io::Error::new(error.kind(), error.to_string()))
                    }
                    GitError::CurrentDirResolution(error) => GitError::CurrentDirResolution(
                        std::io::Error::new(error.kind(), error.to_string()),
                    ),
                    GitError::InvalidOutput(message) => GitError::InvalidOutput(message.clone()),
                    GitError::RepositoryPathResolution { path, error } => {
                        GitError::RepositoryPathResolution {
                            path: path.clone(),
                            error: std::io::Error::new(error.kind(), error.to_string()),
                        }
                    }
                    GitError::RepositoryPathNotDirectory { path } => {
                        GitError::RepositoryPathNotDirectory { path: path.clone() }
                    }
                });
            }
            Ok(self.commits.clone())
        }
    }

    fn make_options(input_mode: InputMode) -> CliOptions {
        CliOptions {
            config_path: None,
            preset: None,
            repository_path: None,
            trust_repo: false,
            input_mode,
        }
    }

    #[test]
    fn test_format_commit_label_cases() {
        for (message, expected) in [
            (
                "feat: add feature\n\nbody\n",
                "commit abc123: feat: add feature",
            ),
            ("\nbody\n", "commit abc123"),
            (
                "feat: add feature  \n\nbody\n",
                "commit abc123: feat: add feature",
            ),
        ] {
            assert_eq!(format_commit_label("abc123", message), expected);
        }
    }

    #[test]
    fn test_run_file_path_and_loader_forwarding() {
        let mut file = NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut file, b"feat: file commit\n").unwrap();
        file.flush().unwrap();
        let file_path = file.into_temp_path();

        let loader = SpyGitLoader {
            seen_args: RefCell::new(None),
            commits: vec![],
        };

        let file_result = run(
            CliOptions {
                config_path: None,
                preset: None,
                repository_path: None,
                trust_repo: false,
                input_mode: InputMode::File {
                    path: file_path.as_os_str().to_string_lossy().into_owned(),
                },
            },
            &loader,
        )
        .unwrap();
        assert_eq!(file_result.parse_failures, 0);
        assert_eq!(file_result.validation_failures, 0);
        assert_eq!(loader.seen_args.borrow().as_ref(), None);
    }

    #[test]
    fn test_run_file_error_reports_path() {
        let loader = SpyGitLoader {
            seen_args: RefCell::new(None),
            commits: vec![],
        };

        let result = run(
            CliOptions {
                config_path: None,
                preset: None,
                repository_path: None,
                trust_repo: false,
                input_mode: InputMode::File {
                    path: "definitely-missing-file.txt".to_string(),
                },
            },
            &loader,
        );

        assert!(
            matches!(result, Err(AppError::FileIo { path, .. }) if path == "definitely-missing-file.txt")
        );
        assert_eq!(loader.seen_args.borrow().as_ref(), None);
    }

    #[test]
    fn test_git_mode_mixed_parse_and_validation_failures() {
        let mut config_file = NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut config_file, b"type:\n  values:\n    - feat\n").unwrap();
        config_file.flush().unwrap();
        let config_path = config_file.into_temp_path();

        let loader = MockGitLoader {
            commits: vec![
                GitCommit {
                    id: "abc123".to_string(),
                    message: "invalid commit without newline".to_string(),
                },
                GitCommit {
                    id: "def456".to_string(),
                    message: "fix: not allowed\n".to_string(),
                },
            ],
            error: None,
        };
        let options = CliOptions {
            config_path: Some(config_path.as_os_str().to_string_lossy().into_owned()),
            preset: None,
            repository_path: None,
            trust_repo: false,
            input_mode: InputMode::Git {
                git_args: vec!["HEAD".to_string()],
            },
        };
        let result = run(options, &loader).unwrap();

        assert_eq!(result.parse_failures, 1);
        assert_eq!(result.validation_failures, 1);
    }

    #[test]
    fn test_git_mode_git_error() {
        let loader = MockGitLoader {
            commits: vec![],
            error: Some(GitError::GitFailed {
                code: Some(128),
                stderr: "fatal: bad revision".to_string(),
            }),
        };
        let options = make_options(InputMode::Git {
            git_args: vec!["HEAD".to_string()],
        });
        let result = run(options, &loader);
        assert!(matches!(result, Err(AppError::Git(_))));
    }

    #[test]
    fn test_git_loader_forwards_flags() {
        let loader = SpyGitLoader {
            seen_args: RefCell::new(None),
            commits: vec![],
        };
        let options = CliOptions {
            config_path: None,
            preset: None,
            repository_path: Some("/repo".to_string()),
            trust_repo: true,
            input_mode: InputMode::Git {
                git_args: vec!["HEAD~1..HEAD".to_string()],
            },
        };

        let _ = run(options, &loader).unwrap();
        assert_eq!(
            loader.seen_args.borrow().as_ref().unwrap(),
            &(
                vec!["HEAD~1..HEAD".to_string()],
                Some("/repo".to_string()),
                true
            )
        );
    }
}
