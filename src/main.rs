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
const EXIT_VALIDATION_ERROR: i32 = 1;
const EXIT_PARSE_ERROR: i32 = 2;
const EXIT_CONFIG_ERROR: i32 = 3;
const EXIT_CLI_USAGE_ERROR: i32 = 4;
const EXIT_IO_ERROR: i32 = 5;
const EXIT_GIT_ERROR: i32 = 6;

fn main() {
    let cli_action = match parse_args(std::env::args().skip(1)) {
        Ok(action) => action,
        Err(error) => {
            eprintln!("Error: {error}");
            process::exit(EXIT_CLI_USAGE_ERROR);
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
            if outcome.parse_failed {
                process::exit(EXIT_PARSE_ERROR);
            }
            if outcome.validation_failed {
                process::exit(EXIT_VALIDATION_ERROR);
            }
            process::exit(EXIT_OK);
        }
        Err(AppError::Config(error)) => {
            eprintln!("Config error: {error}");
            process::exit(EXIT_CONFIG_ERROR);
        }
        Err(AppError::Git(error)) => {
            eprintln!("Error running git: {error}");
            process::exit(EXIT_GIT_ERROR);
        }
        Err(AppError::StdinIo(error)) => {
            eprintln!("Error: Failed to read stdin: {error}");
            process::exit(EXIT_IO_ERROR);
        }
        Err(AppError::FileIo { path, error }) => {
            eprintln!("Error: Failed to read commit message file '{path}': {error}");
            process::exit(EXIT_IO_ERROR);
        }
    }
}
