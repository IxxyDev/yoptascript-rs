use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process;

use crate::Failure;

const TEMP_NAME_ATTEMPTS: u32 = 100;

pub(crate) fn write_stdout(text: &str) -> Result<(), Failure> {
    match io::stdout().lock().write_all(text.as_bytes()) {
        Err(e) if e.kind() != io::ErrorKind::BrokenPipe => Err(Failure::error(format!("Ошибка записи в stdout: {e}"))),
        _ => Ok(()),
    }
}

pub(crate) fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), Failure> {
    let read_only = || Failure::error(format!("Файл '{}' доступен только для чтения", path.display()));
    let failure = |e: io::Error| Failure::error(format!("Не удалось записать файл '{}': {e}", path.display()));

    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    match fs::metadata(&target) {
        Ok(meta) if meta.is_file() => {
            if meta.permissions().readonly() {
                return Err(read_only());
            }
            match OpenOptions::new().write(true).open(&target) {
                Err(e) if e.kind() == io::ErrorKind::PermissionDenied => return Err(read_only()),
                probe => probe.map_err(failure)?,
            };
            match replace_atomically(&target, contents, Some(meta.permissions())) {
                Err(e) if e.kind() == io::ErrorKind::PermissionDenied => fs::write(&target, contents),
                swapped => swapped,
            }
            .map_err(failure)
        }
        Ok(_) => fs::write(&target, contents).map_err(failure),
        Err(_) if fs::symlink_metadata(path).is_ok() => fs::write(path, contents).map_err(failure),
        Err(_) => replace_atomically(&target, contents, None).map_err(failure),
    }
}

fn replace_atomically(target: &Path, contents: &[u8], permissions: Option<fs::Permissions>) -> io::Result<()> {
    let (temp, file) = create_temp_beside(target)?;
    let swapped = fill_and_swap(file, &temp, target, contents, permissions);
    if swapped.is_err() {
        let _ = fs::remove_file(&temp);
    }
    swapped
}

fn create_temp_beside(target: &Path) -> io::Result<(PathBuf, File)> {
    let name = target.file_name().unwrap_or_default().to_string_lossy();
    let dir = target.parent().unwrap_or_else(|| Path::new(""));
    for attempt in 0..TEMP_NAME_ATTEMPTS {
        let temp = dir.join(format!(".{name}.{}.{attempt}.yps-tmp", process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => return Ok((temp, file)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, "не удалось подобрать имя временного файла"))
}

fn fill_and_swap(
    mut file: File,
    temp: &Path,
    target: &Path,
    contents: &[u8],
    permissions: Option<fs::Permissions>,
) -> io::Result<()> {
    if let Some(permissions) = permissions {
        file.set_permissions(permissions)?;
    }
    file.write_all(contents)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temp, target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::Scratch;

    #[test]
    fn creates_a_file_that_does_not_exist_yet() {
        let scratch = Scratch::new("create");
        let target = scratch.path().join("out.js");

        write_atomic(&target, b"console.log(1);\n").expect("запись должна пройти");

        assert_eq!(fs::read(&target).unwrap(), b"console.log(1);\n");
        assert_eq!(scratch.entries(), ["out.js"]);
    }

    #[test]
    fn replaces_existing_contents() {
        let scratch = Scratch::new("replace");
        let target = scratch.path().join("f.yopta");
        fs::write(&target, "старое").unwrap();

        write_atomic(&target, "новое".as_bytes()).expect("запись должна пройти");

        assert_eq!(fs::read_to_string(&target).unwrap(), "новое");
        assert_eq!(scratch.entries(), ["f.yopta"]);
    }

    #[test]
    fn refuses_to_write_over_a_directory() {
        let scratch = Scratch::new("directory");
        let target = scratch.path().join("занято");
        fs::create_dir(&target).unwrap();

        assert!(write_atomic(&target, b"x").is_err());
        assert_eq!(scratch.entries(), ["занято"]);
    }

    #[test]
    fn removes_its_temporary_file_when_the_swap_fails() {
        let scratch = Scratch::new("swap_fails");
        let target = scratch.path().join("занято");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("внутри"), "x").unwrap();

        let result = replace_atomically(&target, b"x", None);

        assert!(result.is_err(), "замена непустого каталога файлом должна провалиться");
        assert_eq!(scratch.entries(), ["занято"]);
    }

    #[test]
    fn two_writers_never_share_a_temporary_file() {
        let scratch = Scratch::new("unique");
        let target = scratch.path().join("f.yopta");

        let (first_path, _first) = create_temp_beside(&target).expect("первый временный файл");
        let (second_path, _second) = create_temp_beside(&target).expect("второй временный файл");

        assert_ne!(first_path, second_path);
    }
}
