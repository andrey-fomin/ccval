use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

const RECORD_SEPARATOR: char = '\u{001e}';
const FIELD_SEPARATOR: char = '\u{001f}';

#[derive(Debug, PartialEq, Clone)]
pub struct GitCommit {
    pub id: String,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum GitError {
    #[error("failed to run git: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to resolve repository path '{path}': {error}")]
    RepositoryPathResolution {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("failed to resolve or canonicalize current working directory: {0}")]
    CurrentDirResolution(#[source] std::io::Error),
    #[error("git command failed with code {code:?}: {stderr}")]
    GitFailed { code: Option<i32>, stderr: String },
    #[error("invalid git output: {0}")]
    InvalidOutput(String),
}

pub trait GitLoader {
    fn load_commits(
        &self,
        args: &[String],
        repository_path: Option<&str>,
        trust_repo: bool,
    ) -> Result<Vec<GitCommit>, GitError>;
}

pub struct GitSubprocess;

impl GitLoader for GitSubprocess {
    fn load_commits(
        &self,
        args: &[String],
        repository_path: Option<&str>,
        trust_repo: bool,
    ) -> Result<Vec<GitCommit>, GitError> {
        load_commits(args, repository_path, trust_repo)
    }
}

fn load_commits(
    git_args: &[String],
    repository_path: Option<&str>,
    trust_repo: bool,
) -> Result<Vec<GitCommit>, GitError> {
    let output = build_git_log_command(git_args, repository_path, trust_repo)?.output()?;

    if !output.status.success() {
        return Err(GitError::GitFailed {
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    parse_git_output(&String::from_utf8_lossy(&output.stdout))
}

fn build_git_log_command(
    git_args: &[String],
    repository_path: Option<&str>,
    trust_repo: bool,
) -> Result<Command, GitError> {
    let current_dir = if trust_repo && repository_path.is_none() {
        Some(std::env::current_dir().map_err(GitError::CurrentDirResolution)?)
    } else {
        None
    };

    build_git_log_command_with_current_dir(
        git_args,
        repository_path,
        trust_repo,
        current_dir.as_deref().unwrap_or_else(|| Path::new(".")),
    )
}

fn build_git_log_command_with_current_dir(
    git_args: &[String],
    repository_path: Option<&str>,
    trust_repo: bool,
    current_dir: &Path,
) -> Result<Command, GitError> {
    let format = "%x1e%H%x1f%B";
    let mut cmd = Command::new("git");

    let resolved_repository_path = if trust_repo {
        repository_path
            .map(|path| {
                resolve_repository_path(path).map_err(|error| GitError::RepositoryPathResolution {
                    path: path.to_string(),
                    error,
                })
            })
            .transpose()?
    } else {
        None
    };

    if let Some(path) = resolved_repository_path.as_ref() {
        cmd.arg("-C").arg(path);
    } else if let Some(path) = repository_path {
        cmd.args(["-C", path]);
    }

    if trust_repo {
        let safe_directory = match resolved_repository_path {
            Some(path) => path,
            None => current_dir
                .canonicalize()
                .map_err(GitError::CurrentDirResolution)?,
        };
        cmd.arg("-c").arg(format!(
            "safe.directory={}",
            safe_directory.to_string_lossy()
        ));
    }

    cmd.arg("log")
        .args(git_args)
        .arg(format!("--format={format}"));

    Ok(cmd)
}

fn resolve_repository_path(path: &str) -> Result<PathBuf, std::io::Error> {
    Path::new(path).canonicalize()
}

fn parse_git_output(output: &str) -> Result<Vec<GitCommit>, GitError> {
    let mut commits = Vec::new();

    for record in output
        .split(RECORD_SEPARATOR)
        .filter(|record| !record.is_empty())
    {
        let Some((id, message)) = record.split_once(FIELD_SEPARATOR) else {
            return Err(GitError::InvalidOutput(
                "missing commit field separator".to_string(),
            ));
        };

        let message = message
            .strip_suffix('\n')
            .expect("Git %B output should end with newline");

        commits.push(GitCommit {
            id: id.to_string(),
            message: message.to_string(),
        });
    }

    Ok(commits)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::{GitCommit, GitError, build_git_log_command_with_current_dir, parse_git_output};

    fn command_args(command: &std::process::Command) -> Vec<String> {
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn parse_git_output_empty() {
        assert_eq!(parse_git_output("").unwrap(), Vec::<GitCommit>::new());
    }

    #[test]
    fn parse_git_output_single_commit() {
        let commits = parse_git_output("\u{001e}abc123\u{001f}feat: subject\n\n").unwrap();
        assert_eq!(
            commits,
            vec![GitCommit {
                id: "abc123".to_string(),
                message: "feat: subject\n".to_string(),
            }]
        );
    }

    #[test]
    fn parse_git_output_multiple_commits() {
        let commits = parse_git_output(
            "\u{001e}abc123\u{001f}feat: subject\n\n\u{001e}def456\u{001f}fix: bug\n\n",
        )
        .unwrap();
        assert_eq!(
            commits,
            vec![
                GitCommit {
                    id: "abc123".to_string(),
                    message: "feat: subject\n".to_string(),
                },
                GitCommit {
                    id: "def456".to_string(),
                    message: "fix: bug\n".to_string(),
                },
            ]
        );
    }

    #[test]
    fn parse_git_output_invalid_record() {
        let error = parse_git_output("\u{001e}abc123").unwrap_err();
        assert!(matches!(error, GitError::InvalidOutput(_)));
    }

    #[test]
    fn build_git_log_command_without_trust_repo_uses_raw_repository_path() {
        let git_args = vec!["HEAD".to_string()];
        let command = build_git_log_command_with_current_dir(
            &git_args,
            Some("relative/repo"),
            false,
            std::env::temp_dir().as_path(),
        )
        .unwrap();

        assert_eq!(command.get_program(), OsStr::new("git"));
        assert_eq!(
            command_args(&command),
            vec![
                "-C".to_string(),
                "relative/repo".to_string(),
                "log".to_string(),
                "HEAD".to_string(),
                "--format=%x1e%H%x1f%B".to_string(),
            ]
        );
    }

    #[test]
    fn build_git_log_command_with_trust_repo_uses_canonical_repository_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_path = temp_dir.path().join("repo");
        std::fs::create_dir(&repo_path).unwrap();

        let current_dir = std::env::temp_dir();
        let git_args = vec!["HEAD".to_string()];
        let command = build_git_log_command_with_current_dir(
            &git_args,
            Some(repo_path.to_str().unwrap()),
            true,
            current_dir.as_path(),
        )
        .unwrap();

        let canonical_repo_path = repo_path.canonicalize().unwrap();
        assert_eq!(
            command_args(&command),
            vec![
                "-C".to_string(),
                canonical_repo_path.to_string_lossy().into_owned(),
                "-c".to_string(),
                format!("safe.directory={}", canonical_repo_path.to_string_lossy()),
                "log".to_string(),
                "HEAD".to_string(),
                "--format=%x1e%H%x1f%B".to_string(),
            ]
        );
    }

    #[test]
    fn build_git_log_command_with_trust_repo_uses_canonical_current_dir() {
        let current_dir = tempfile::tempdir().unwrap();
        let git_args = vec!["HEAD~1..HEAD".to_string()];
        let command =
            build_git_log_command_with_current_dir(&git_args, None, true, current_dir.path())
                .unwrap();

        let canonical_current_dir = current_dir.path().canonicalize().unwrap();
        assert_eq!(
            command_args(&command),
            vec![
                "-c".to_string(),
                format!("safe.directory={}", canonical_current_dir.to_string_lossy()),
                "log".to_string(),
                "HEAD~1..HEAD".to_string(),
                "--format=%x1e%H%x1f%B".to_string(),
            ]
        );
    }

    #[test]
    fn build_git_log_command_reports_repo_path_resolution_errors() {
        let temp_dir = tempfile::tempdir().unwrap();
        let missing_repo = temp_dir.path().join("missing-repo");
        let error = build_git_log_command_with_current_dir(
            &[],
            Some(missing_repo.to_str().unwrap()),
            true,
            std::env::temp_dir().as_path(),
        )
        .unwrap_err();

        assert!(matches!(error, GitError::RepositoryPathResolution { .. }));
    }

    #[test]
    fn build_git_log_command_reports_current_dir_resolution_errors() {
        let temp_dir = tempfile::tempdir().unwrap();
        let invalid_current_dir = temp_dir.path().join("missing-current-dir");
        let error = build_git_log_command_with_current_dir(&[], None, true, &invalid_current_dir)
            .unwrap_err();

        assert!(matches!(error, GitError::CurrentDirResolution(_)));
    }

    #[test]
    fn build_git_log_command_untrusted_no_repo_has_no_safe_directory() {
        let command = build_git_log_command_with_current_dir(
            &["HEAD".to_string()],
            None,
            false,
            std::env::temp_dir().as_path(),
        )
        .unwrap();

        assert!(
            !command_args(&command)
                .iter()
                .any(|arg| arg.contains("safe.directory"))
        );
    }
}
