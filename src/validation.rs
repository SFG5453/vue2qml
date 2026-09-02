use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::{Error, ErrorKind, Result};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct QmlValidationReport {
    pub files_checked: usize,
    pub entrypoint_smoked: bool,
}

pub fn validate_qml_tree(root: impl AsRef<Path>) -> Result<QmlValidationReport> {
    let root = root.as_ref();
    let mut files = Vec::new();
    collect_qml_files(root, &mut files)?;
    files.sort();
    if files.is_empty() {
        return Err(Error::new(
            ErrorKind::Validation,
            "QML output tree does not contain any .qml files",
        )
        .at(root));
    }
    for file in &files {
        let output = run_tool("qmlformat", ["-n"], [file.as_path()])?;
        require_success("qmlformat", file, &output, false)?;
    }
    let output = run_qmllint(root, &files)?;
    require_success("qmllint", root, &output, true)?;
    let entrypoint = root.join("src/App.qml");
    let entrypoint_smoked = if entrypoint.is_file() {
        smoke_entrypoint(root, &entrypoint)?;
        true
    } else {
        false
    };
    Ok(QmlValidationReport {
        files_checked: files.len(),
        entrypoint_smoked,
    })
}

fn collect_qml_files(directory: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
    let entries = fs::read_dir(directory).map_err(|error| Error::io(error, directory))?;
    for entry in entries {
        let entry = entry.map_err(|error| Error::io(error, directory))?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| Error::io(error, &path))?;
        if file_type.is_dir() {
            collect_qml_files(&path, output)?;
        } else if file_type.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("qml")
        {
            output.push(path);
        }
    }
    Ok(())
}

fn run_qmllint(root: &Path, files: &[PathBuf]) -> Result<Output> {
    let mut command = Command::new("qmllint");
    command.arg("-I").arg(root).args(files);
    command
        .output()
        .map_err(|error| tool_error("qmllint", error))
}

fn run_tool<'path>(
    tool: &str,
    arguments: impl IntoIterator<Item = &'static str>,
    paths: impl IntoIterator<Item = &'path Path>,
) -> Result<Output> {
    Command::new(tool)
        .args(arguments)
        .args(paths)
        .output()
        .map_err(|error| tool_error(tool, error))
}

fn require_success(
    tool: &str,
    path: &Path,
    output: &Output,
    diagnostics_are_errors: bool,
) -> Result<()> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let diagnostics = [stdout.trim(), stderr.trim()]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if !output.status.success() || (diagnostics_are_errors && !diagnostics.is_empty()) {
        let detail = if diagnostics.is_empty() {
            format!(
                "{tool} exited with {} and emitted no diagnostic",
                output.status
            )
        } else {
            format!("{tool} reported:\n{diagnostics}")
        };
        return Err(Error::new(ErrorKind::Validation, detail).at(path));
    }
    Ok(())
}

fn smoke_entrypoint(root: &Path, entrypoint: &Path) -> Result<()> {
    let mut child = spawn_qmlscene(root, entrypoint)?;
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child
                    .wait_with_output()
                    .map_err(|error| tool_error("qmlscene", error))?;
                if status.success() {
                    return Ok(());
                }
                return require_success("qmlscene", entrypoint, &output, true);
            }
            Ok(None) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(25));
            }
            Ok(None) => {
                child
                    .kill()
                    .map_err(|error| tool_error("qmlscene", error))?;
                let output = child
                    .wait_with_output()
                    .map_err(|error| tool_error("qmlscene", error))?;
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.trim().is_empty() {
                    return Ok(());
                }
                return Err(Error::new(
                    ErrorKind::Validation,
                    format!("qmlscene emitted runtime diagnostics:\n{}", stderr.trim()),
                )
                .at(entrypoint));
            }
            Err(error) => return Err(tool_error("qmlscene", error)),
        }
    }
}

fn spawn_qmlscene(root: &Path, entrypoint: &Path) -> Result<std::process::Child> {
    let configure = |command: &mut Command| {
        command
            .arg("-I")
            .arg(root)
            .arg(entrypoint)
            .env("QT_QPA_PLATFORM", "offscreen")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
    };
    let mut preferred = Command::new("qmlscene6");
    configure(&mut preferred);
    match preferred.spawn() {
        Ok(child) => Ok(child),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut fallback = Command::new("qmlscene");
            configure(&mut fallback);
            fallback
                .spawn()
                .map_err(|error| tool_error("qmlscene", error))
        }
        Err(error) => Err(tool_error("qmlscene6", error)),
    }
}

fn tool_error(tool: &str, error: io::Error) -> Error {
    Error::new(
        ErrorKind::Validation,
        format!("could not run {tool}: {error}"),
    )
}
