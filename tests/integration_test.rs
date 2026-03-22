use std::io::ErrorKind;
use std::io::Write;
use std::process::{Command, Stdio};

use tempfile::NamedTempFile;

fn binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_ccval")
}

fn run_with_stdin(args: &[&str], stdin: &str) -> (String, String, i32) {
    let mut cmd = Command::new(binary_path());
    cmd.args(args);
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn process");

    {
        let mut stdin_handle = child.stdin.take().expect("Failed to open stdin");
        match stdin_handle.write_all(stdin.as_bytes()) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::BrokenPipe => {}
            Err(error) => panic!("Failed to write to stdin: {error}"),
        }
    }

    let output = child.wait_with_output().expect("Failed to read output");
    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.code().unwrap_or(-1),
    )
}

fn run_with_file(args: &[&str], file_content: &str) -> (String, String, i32) {
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    temp_file
        .write_all(file_content.as_bytes())
        .expect("Failed to write temp file");
    temp_file.flush().expect("Failed to flush temp file");
    let temp_path = temp_file.into_temp_path();

    let mut cmd = Command::new(binary_path());
    cmd.args(args);
    cmd.arg("--file");
    cmd.arg(temp_path.as_os_str());

    let output = cmd.output().expect("Failed to execute process");

    (
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
        output.status.code().unwrap_or(-1),
    )
}

#[test]
fn valid_commit_exits_zero() {
    let (_, _, code) = run_with_stdin(&["--stdin"], "feat: add new feature\n");
    assert_eq!(code, 0);
}

#[test]
fn valid_commit_with_scope_exits_zero() {
    let (_, _, code) = run_with_stdin(&["--stdin"], "feat(api): add endpoint\n");
    assert_eq!(code, 0);
}

#[test]
fn valid_commit_with_body_exits_zero() {
    let (_, _, code) = run_with_stdin(&["--stdin"], "feat: add feature\n\nThis is the body.\n");
    assert_eq!(code, 0);
}

#[test]
fn default_git_mode_exits_zero() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let temp_path = temp_dir.path();

    let git_status = Command::new("git")
        .args(["init"])
        .current_dir(temp_path)
        .status()
        .expect("Failed to run git init");
    assert!(git_status.success());

    let config_status = Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp_path)
        .status()
        .expect("Failed to run git config");
    assert!(config_status.success());

    let config_status = Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(temp_path)
        .status()
        .expect("Failed to run git config");
    assert!(config_status.success());

    std::fs::write(temp_path.join("test.txt"), "test").expect("Failed to write file");

    let add_status = Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(temp_path)
        .status()
        .expect("Failed to run git add");
    assert!(add_status.success());

    let commit_status = Command::new("git")
        .args([
            "commit",
            "--no-gpg-sign",
            "--no-verify",
            "-m",
            "feat: default git commit",
        ])
        .current_dir(temp_path)
        .status()
        .expect("Failed to run git commit");
    assert!(commit_status.success());

    let output = Command::new(binary_path())
        .current_dir(temp_path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn invalid_type_exits_content_invalid() {
    let (_, stderr, code) = run_with_stdin(&["--stdin"], "feat\n");
    assert_eq!(code, 65);
    assert!(stderr.contains("Parsing error"));
}

#[test]
fn missing_newline_exits_content_invalid() {
    let (_, stderr, code) = run_with_stdin(&["--stdin"], "feat: missing newline");
    assert_eq!(code, 65);
    assert!(stderr.contains("newline"));
}

#[test]
fn missing_colon_exits_content_invalid() {
    let (_, stderr, code) = run_with_stdin(&["--stdin"], "feat missing colon\n");
    assert_eq!(code, 65);
    assert!(stderr.contains("colon"));
}

#[test]
fn help_flag_exits_zero() {
    let (stdout, _, code) = run_with_stdin(&["--help"], "");
    assert_eq!(code, 0);
    assert!(stdout.contains("Usage:"));
}

#[test]
fn file_mode_invalid_commit() {
    let (_, stderr, code) = run_with_file(&[], "invalid\n");
    assert_eq!(code, 65);
    assert!(stderr.contains("Parsing error"));
}

#[test]
fn custom_config_path() {
    let config_content = "type:\n  values:\n    - custom\n";
    let mut config_file = NamedTempFile::new().expect("Failed to create temp config file");
    config_file
        .write_all(config_content.as_bytes())
        .expect("Failed to write config");
    config_file.flush().expect("Failed to flush config file");

    let (_, _, code_valid) = run_with_stdin(
        &["-c", config_file.path().to_str().unwrap(), "--stdin"],
        "custom: valid type\n",
    );
    assert_eq!(code_valid, 0);

    let (_, stderr, code_invalid) = run_with_stdin(
        &["-c", config_file.path().to_str().unwrap(), "--stdin"],
        "feat: invalid type\n",
    );
    assert_eq!(code_invalid, 65);
    assert!(stderr.contains("not in allowed values"));
}

#[test]
fn preset_flag_applies_strict_validation() {
    let message = format!("feat: {}\n", "a".repeat(60));
    let (_, stderr, code) = run_with_stdin(&["-p", "strict", "--stdin"], &message);
    assert_eq!(code, 65);
    assert!(stderr.contains("Validation error"));
}

#[test]
fn preset_flag_allows_default_validation() {
    let (_, _, code) = run_with_stdin(&["-p", "default", "--stdin"], "feat: add feature\n");
    assert_eq!(code, 0);
}

#[test]
fn config_can_clear_preset_regexes() {
    let config_content = "preset: strict\ndescription:\n  regexes: []\n";
    let config_file = tempfile::NamedTempFile::new().expect("Failed to create temp config file");
    std::fs::write(config_file.path(), config_content).expect("Failed to write config");

    // This message ends with a period, which violates strict preset description regex
    let (_, _, code) = run_with_stdin(
        &["-c", config_file.path().to_str().unwrap(), "--stdin"],
        "feat: add feature.\n",
    );
    // Should pass because regexes were cleared
    assert_eq!(code, 0);
}

#[test]
fn config_keeps_preset_regexes_when_override_omits_regexes() {
    let config_content = "preset: strict\ndescription:\n  max-length: 100\n";
    let config_file = tempfile::NamedTempFile::new().expect("Failed to create temp config file");
    std::fs::write(config_file.path(), config_content).expect("Failed to write config");

    let (_, stderr, code) = run_with_stdin(
        &["-c", config_file.path().to_str().unwrap(), "--stdin"],
        "feat: add feature.\n",
    );
    assert_eq!(code, 65);
    assert!(stderr.contains("does not match regex"));
}

#[test]
fn stdin_mode_explicit() {
    let (_, _, code1) = run_with_stdin(&["--stdin"], "docs: update readme\n");
    let (_, _, code2) = run_with_stdin(&["--stdin"], "feat: new feature\n");
    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
}

#[test]
fn non_printable_char_rejected() {
    let (_, stderr, code) = run_with_stdin(&["--stdin"], "feat: tab\there\n");
    assert_eq!(code, 65);
    assert!(stderr.contains("Non-printable"));
}

#[test]
fn repository_flag_validates_from_alternate_repo() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let temp_path = temp_dir.path();

    let git_status = Command::new("git")
        .args(["init"])
        .current_dir(temp_path)
        .status()
        .expect("Failed to run git init");
    assert!(git_status.success());

    let config_status = Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(temp_path)
        .status()
        .expect("Failed to run git config");
    assert!(config_status.success());

    let config_status = Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(temp_path)
        .status()
        .expect("Failed to run git config");
    assert!(config_status.success());

    std::fs::write(temp_path.join("test.txt"), "test").expect("Failed to write file");

    let add_status = Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(temp_path)
        .status()
        .expect("Failed to run git add");
    assert!(add_status.success());

    let commit_status = Command::new("git")
        .args([
            "commit",
            "--no-gpg-sign",
            "--no-verify",
            "-m",
            "feat: test commit\n",
        ])
        .current_dir(temp_path)
        .status()
        .expect("Failed to run git commit");
    assert!(commit_status.success());

    let mut cmd = Command::new(binary_path());
    cmd.arg("-r").arg(temp_path);
    let output = cmd.output().expect("Failed to execute process");

    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn missing_config_exits_config_error() {
    let (_, stderr, code) = run_with_stdin(
        &["--config", "definitely-missing-config.yaml", "--stdin"],
        "feat: valid commit\n",
    );
    assert_eq!(code, 78);
    assert!(stderr.contains("Config error"));
}

#[test]
fn missing_file_exits_input_unavailable() {
    let (_, stderr, code) = run_with_stdin(&["--file", "definitely-missing.txt"], "");
    assert_eq!(code, 66);
    assert!(stderr.contains("Failed to read commit message file"));
}

#[test]
fn invalid_cli_usage_exits_usage_error() {
    let (_, stderr, code) = run_with_stdin(&["--unknown"], "");
    assert_eq!(code, 64);
    assert!(stderr.contains("Error: unknown argument"));
}

#[test]
fn file_mode_valid_commit() {
    let (_, _, code) = run_with_file(&[], "fix: bug fix\n");
    assert_eq!(code, 0);
}

#[test]
fn git_failure_exits_input_unavailable() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

    let mut cmd = Command::new(binary_path());
    cmd.arg("-r").arg(temp_dir.path().join("missing-repo"));
    let output = cmd.output().expect("Failed to execute process");

    assert_eq!(output.status.code(), Some(66));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Error running git"));
}

#[test]
fn repository_flag_to_file_exits_usage_error() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");

    let output = Command::new(binary_path())
        .arg("-r")
        .arg(temp_file.path())
        .output()
        .expect("Failed to execute process");

    assert_eq!(output.status.code(), Some(64));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Error running git"));
    assert!(stderr.contains("not a directory"));
}

#[test]
fn file_mode_reports_validation_failure() {
    let (_, stderr, code) = run_with_file(&[], "foo: invalid type\n");

    assert_eq!(code, 65);
    assert!(stderr.contains("Validation error"));
}
