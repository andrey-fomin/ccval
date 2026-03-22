mod app;
mod cli;
mod config;
mod git;
mod parser;
mod validator;

use std::process;

use app::{AppError, run};
use cli::{CliAction, HELP_TEXT, parse_args};
use git::GitSubprocess;

const EXIT_OK: i32 = 0;
const EXIT_USAGE_ERROR: i32 = 64;
const EXIT_CONTENT_INVALID: i32 = 65;
const EXIT_INPUT_UNAVAILABLE: i32 = 66;
const EXIT_PERMISSION_ERROR: i32 = 77;
const EXIT_CONFIG_ERROR: i32 = 78;
const EXIT_IO_ERROR: i32 = 74;
const EXIT_INTERNAL_ERROR: i32 = 70;

fn io_error_exit_code(error: &std::io::Error) -> i32 {
    match error.kind() {
        std::io::ErrorKind::NotFound => EXIT_INPUT_UNAVAILABLE,
        std::io::ErrorKind::PermissionDenied => EXIT_PERMISSION_ERROR,
        std::io::ErrorKind::InvalidInput => EXIT_USAGE_ERROR,
        _ => EXIT_IO_ERROR,
    }
}

fn git_failed_exit_code(code: Option<i32>, stderr: &str) -> i32 {
    match code {
        Some(128 | 129) if stderr.contains("dubious ownership") => EXIT_PERMISSION_ERROR,
        Some(128 | 129) if stderr.contains("not a git repository") => EXIT_INPUT_UNAVAILABLE,
        Some(128 | 129)
            if stderr.contains("unknown revision") || stderr.contains("ambiguous argument") =>
        {
            EXIT_USAGE_ERROR
        }
        Some(128 | 129) => EXIT_USAGE_ERROR,
        Some(_) => EXIT_IO_ERROR,
        None if stderr.contains("dubious ownership") => EXIT_PERMISSION_ERROR,
        None if stderr.contains("not a git repository") => EXIT_INPUT_UNAVAILABLE,
        None if stderr.contains("unknown revision") || stderr.contains("ambiguous argument") => {
            EXIT_USAGE_ERROR
        }
        None => EXIT_IO_ERROR,
    }
}

fn main() {
    let cli_action = match parse_args(std::env::args().skip(1)) {
        Ok(action) => action,
        Err(error) => {
            eprintln!("Error: {error}");
            process::exit(EXIT_USAGE_ERROR);
        }
    };

    let options = match cli_action {
        CliAction::ShowHelp => {
            print!("{HELP_TEXT}");
            return;
        }
        CliAction::Run(options) => options,
    };

    match run(options, &GitSubprocess) {
        Ok(outcome) => {
            if outcome.parse_failures > 0 || outcome.validation_failures > 0 {
                process::exit(EXIT_CONTENT_INVALID);
            }
            process::exit(EXIT_OK);
        }
        Err(AppError::Config(error)) => {
            eprintln!("Config error: {error}");
            process::exit(EXIT_CONFIG_ERROR);
        }
        Err(AppError::Git(error)) => {
            let exit_code = match &error {
                git::GitError::RepositoryPathResolution { error, .. } => io_error_exit_code(error),
                git::GitError::RepositoryPathNotDirectory { .. } => EXIT_USAGE_ERROR,
                git::GitError::CurrentDirResolution(e) => io_error_exit_code(e),
                git::GitError::GitFailed { code, stderr } => git_failed_exit_code(*code, stderr),
                git::GitError::InvalidOutput(_) => EXIT_INTERNAL_ERROR,
                git::GitError::Io(e) => io_error_exit_code(e),
            };
            eprintln!("Error running git: {error}");
            process::exit(exit_code);
        }
        Err(AppError::StdinIo(error)) => {
            eprintln!("Error: Failed to read stdin: {error}");
            process::exit(io_error_exit_code(&error));
        }
        Err(AppError::FileIo { path, error }) => {
            eprintln!("Error: Failed to read commit message file '{path}': {error}");
            process::exit(io_error_exit_code(&error));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_failed_exit_code_maps_revision_errors_to_usage() {
        let stderr =
            "fatal: ambiguous argument 'badrev': unknown revision or path not in the working tree.";

        assert_eq!(EXIT_USAGE_ERROR, git_failed_exit_code(Some(128), stderr));
        assert_eq!(EXIT_USAGE_ERROR, git_failed_exit_code(Some(129), stderr));
        assert_eq!(EXIT_USAGE_ERROR, git_failed_exit_code(None, stderr));
    }

    #[test]
    fn git_failed_exit_code_maps_not_a_git_repository_to_input_unavailable() {
        let stderr = "fatal: not a git repository (or any of the parent directories): .git";

        assert_eq!(
            EXIT_INPUT_UNAVAILABLE,
            git_failed_exit_code(Some(128), stderr)
        );
        assert_eq!(EXIT_INPUT_UNAVAILABLE, git_failed_exit_code(None, stderr));
    }

    #[test]
    fn git_failed_exit_code_maps_dubious_ownership_to_permission_error() {
        let stderr = "fatal: detected dubious ownership in repository at '/tmp/repo'";

        assert_eq!(
            EXIT_PERMISSION_ERROR,
            git_failed_exit_code(Some(128), stderr)
        );
        assert_eq!(EXIT_PERMISSION_ERROR, git_failed_exit_code(None, stderr));
    }

    #[test]
    fn git_failed_exit_code_falls_back_to_io_error() {
        let stderr = "fatal: something unexpected happened";

        assert_eq!(EXIT_IO_ERROR, git_failed_exit_code(Some(1), stderr));
        assert_eq!(EXIT_IO_ERROR, git_failed_exit_code(None, stderr));
    }

    #[test]
    fn git_failed_exit_code_maps_unknown_revision_issues_to_usage_error() {
        let stderr = "fatal: unknown revision or path not in the working tree.";

        assert_eq!(EXIT_USAGE_ERROR, git_failed_exit_code(Some(128), stderr));
        assert_eq!(EXIT_USAGE_ERROR, git_failed_exit_code(Some(129), stderr));
    }

    #[test]
    fn io_error_exit_code_maps_invalid_input_to_usage_error() {
        let error = std::io::Error::new(std::io::ErrorKind::InvalidInput, "bad input");

        assert_eq!(EXIT_USAGE_ERROR, io_error_exit_code(&error));
    }
}
