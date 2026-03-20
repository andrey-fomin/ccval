use crate::cli::{CliOptions, InputMode};
use crate::config::{Config, ConfigError};
use crate::git::{GitError, GitLoader};
use crate::{parser, validator};

#[derive(Debug)]
pub struct RunOutcome {
    pub parse_failed: bool,
    pub validation_failed: bool,
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

    let mut parse_failed = false;
    let mut validation_failed = false;

    for input in inputs {
        match parser::parse(&input.message) {
            Ok(commit) => {
                let errors = validator::validate(&commit, &config);
                if !errors.is_empty() {
                    validation_failed = true;
                    if input.label != "stdin" {
                        eprintln!("{}:", input.label);
                    }
                    for error in errors {
                        eprintln!("Validation error: {}", error);
                    }
                }
            }
            Err(error) => {
                parse_failed = true;
                if input.label != "stdin" {
                    eprintln!("{}:", input.label);
                }
                eprintln!("{}", error);
            }
        }
    }

    Ok(RunOutcome {
        parse_failed,
        validation_failed,
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
            label: "stdin".to_string(),
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
                label: commit.id,
                message: commit.message,
            })
            .collect()),
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
                return Err(GitError::GitFailed {
                    code: match err {
                        GitError::GitFailed { code, .. } => *code,
                        _ => None,
                    },
                    stderr: match err {
                        GitError::GitFailed { stderr, .. } => stderr.clone(),
                        GitError::Io(e) => e.to_string(),
                        GitError::InvalidOutput(s) => s.clone(),
                        GitError::RepositoryPathResolution { error, .. } => error.to_string(),
                        GitError::CurrentDirResolution(e) => e.to_string(),
                    },
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
    fn test_git_mode_empty_commits() {
        let loader = MockGitLoader {
            commits: vec![],
            error: None,
        };
        let options = make_options(InputMode::Git {
            git_args: vec!["HEAD".to_string()],
        });
        let result = run(options, &loader).unwrap();
        assert!(!result.parse_failed);
        assert!(!result.validation_failed);
    }

    #[test]
    fn test_git_mode_valid_commit() {
        let loader = MockGitLoader {
            commits: vec![GitCommit {
                id: "abc123".to_string(),
                message: "feat: valid commit\n".to_string(),
            }],
            error: None,
        };
        let options = make_options(InputMode::Git {
            git_args: vec!["HEAD".to_string()],
        });
        let result = run(options, &loader).unwrap();
        assert!(!result.parse_failed);
        assert!(!result.validation_failed);
    }

    #[test]
    fn test_git_mode_parse_error() {
        let loader = MockGitLoader {
            commits: vec![GitCommit {
                id: "abc123".to_string(),
                message: "invalid commit without newline".to_string(),
            }],
            error: None,
        };
        let options = make_options(InputMode::Git {
            git_args: vec!["HEAD".to_string()],
        });
        let result = run(options, &loader).unwrap();
        assert!(result.parse_failed);
    }

    #[test]
    fn test_git_mode_validation_error_with_config() {
        let config_content = "type:\n  values:\n    - feat\n    - fix\n";
        let config_file = std::env::temp_dir().join("ccval_test_config.yaml");
        std::fs::write(&config_file, config_content).unwrap();
        let config_path = config_file.to_str().unwrap().to_string();

        let loader = MockGitLoader {
            commits: vec![GitCommit {
                id: "abc123".to_string(),
                message: "invalid_type: description\n".to_string(),
            }],
            error: None,
        };
        let options = CliOptions {
            config_path: Some(config_path),
            preset: None,
            repository_path: None,
            trust_repo: false,
            input_mode: InputMode::Git {
                git_args: vec!["HEAD".to_string()],
            },
        };
        let result = run(options, &loader).unwrap();

        let _ = std::fs::remove_file(&config_file);

        assert!(!result.parse_failed);
        assert!(result.validation_failed);
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
}
