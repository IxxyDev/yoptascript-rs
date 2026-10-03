#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

pub const BIN: &str = env!("CARGO_BIN_EXE_yps-cli");

pub struct Run {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
    pub signal: Option<i32>,
}

#[cfg(unix)]
fn signal_of(status: ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
fn signal_of(_status: ExitStatus) -> Option<i32> {
    None
}

pub fn run(args: &[&str], stdin: &str) -> Run {
    run_command(Command::new(BIN).args(args), stdin.as_bytes())
}

pub fn run_in(dir: &Path, args: &[&str], stdin: &str) -> Run {
    run_command(Command::new(BIN).args(args).current_dir(dir), stdin.as_bytes())
}

pub fn run_command(command: &mut Command, stdin: &[u8]) -> Run {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("не удалось запустить yps-cli");

    {
        let mut handle = child.stdin.take().expect("stdin недоступен");
        let _ = handle.write_all(stdin);
    }

    let out = child.wait_with_output().expect("ожидание завершения yps-cli");
    Run {
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        code: out.status.code().unwrap_or(-1),
        signal: signal_of(out.status),
    }
}

pub fn run_until_first_line(args: &[&str], stdin: &str) -> Run {
    let mut child = Command::new(BIN)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("не удалось запустить yps-cli");

    {
        let mut handle = child.stdin.take().expect("stdin недоступен");
        let _ = handle.write_all(stdin.as_bytes());
    }

    let mut first_line = String::new();
    {
        let mut reader = BufReader::new(child.stdout.take().expect("stdout недоступен"));
        reader.read_line(&mut first_line).expect("чтение первой строки");
    }

    let out = child.wait_with_output().expect("ожидание завершения yps-cli");
    Run {
        stdout: first_line,
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        code: out.status.code().unwrap_or(-1),
        signal: signal_of(out.status),
    }
}

pub struct Workspace {
    dir: PathBuf,
}

impl Workspace {
    pub fn new(tag: &str) -> Workspace {
        let dir = std::env::temp_dir().join(format!("yps_cli_it_{}_{}", tag, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("создать временный каталог");
        Workspace { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.dir.join(name);
        std::fs::write(&path, contents).expect("записать тестовый файл");
        path
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    pub fn entries(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.dir)
            .expect("прочитать временный каталог")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
