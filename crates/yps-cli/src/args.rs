use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

pub(crate) const HELP: &str = "Использование: yps [--vm] ФАЙЛ [АРГУМЕНТЫ...]
       yps [--vm] -e \"код\" [-- АРГУМЕНТЫ...]
       yps [--vm] - [АРГУМЕНТЫ...]
       yps repl
       yps fmt [--write|-w] [--check] [--source-map] <файл.yopta>
       yps ast <файл.yopta>
       yps disasm <файл.yopta>
       yps lint <файл.yopta>
       yps transpile <файл.yopta> [-o файл.js]

Выполнение программы:
  yps ФАЙЛ                  выполнить файл на дереве интерпретации
  yps --vm ФАЙЛ             выполнить файл на байткодовой VM
  yps -e \"код\", --eval \"код\"  выполнить код, переданный строкой
  yps -                     выполнить код, прочитанный из stdin
  yps repl                  запустить интерактивный REPL
  yps                       без аргументов — тоже REPL

  Источник программы один: ФАЙЛ, -e или «-». Флаги yps идут до него.
  Всё после ФАЙЛ или «-» передаётся программе; для -e АРГУМЕНТЫ
  отделяются через «--». Программа видит их в Процесс.аргументы,
  первым элементом идёт ФАЙЛ, «-e» или «-».
  Чтобы запустить файл, имя которого начинается с «-»: yps -- ФАЙЛ.

Форматирование:
  yps fmt <файл.yopta>              напечатать отформатированный код в stdout
  yps fmt <файл.yopta> --write|-w   переписать файл на месте
  yps fmt <файл.yopta> --check      проверить, отформатирован ли файл (код выхода)
  yps fmt <файл.yopta> --write --source-map
                                    вдобавок записать source map в <файл.yopta>.map
  --check нельзя сочетать с --write и --source-map

Отладка:
  yps ast <файл.yopta>      напечатать дерево разбора (AST) файла
  yps disasm <файл.yopta>   напечатать дизассемблированный байткод VM
  yps lint <файл.yopta>     проверить файл линтером (код выхода 1 при находках)

Транспиляция:
  yps transpile <файл.yopta>             напечатать JS в stdout
  yps transpile <файл.yopta> -o файл.js  записать JS в файл

Прочее:
  -h, --help       показать эту справку (после подкоманды — справку по ней)
  -V, --version    показать версию

Коды выхода:
  0   успех
  1   ошибка в программе или файле, находки линтера, расхождение в fmt --check
  2   неверные аргументы командной строки
  70  внутренняя ошибка";

pub(crate) const REPL_USAGE: &str = "Использование: yps repl";
pub(crate) const FMT_USAGE: &str = "Использование: yps fmt [--write|-w] [--check] [--source-map] <файл.yopta>";
pub(crate) const AST_USAGE: &str = "Использование: yps ast <файл.yopta>";
pub(crate) const DISASM_USAGE: &str = "Использование: yps disasm <файл.yopta>";
pub(crate) const LINT_USAGE: &str = "Использование: yps lint <файл.yopta>";
pub(crate) const TRANSPILE_USAGE: &str = "Использование: yps transpile <файл.yopta> [-o файл.js]";

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Command {
    Help(&'static str),
    Version,
    Repl,
    Run { source: RunSource, use_vm: bool, script_args: Vec<String> },
    Fmt { file: PathBuf, mode: FmtMode, source_map: bool },
    Ast { file: PathBuf },
    Disasm { file: PathBuf },
    Lint { file: PathBuf },
    Transpile { file: PathBuf, output: Option<PathBuf> },
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RunSource {
    File(PathBuf),
    Eval(String),
    Stdin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FmtMode {
    Print,
    Write,
    Check,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct UsageError(pub(crate) String);

enum Word<'a> {
    Flag(&'a str),
    Positional(&'a OsStr),
}

struct Words<'a> {
    rest: &'a [OsString],
    literal: bool,
}

impl<'a> Words<'a> {
    const fn new(args: &'a [OsString]) -> Self {
        Self { rest: args, literal: false }
    }

    fn value(&mut self) -> Option<&'a OsString> {
        let (first, rest) = self.rest.split_first()?;
        self.rest = rest;
        Some(first)
    }

    fn finish(self) -> Result<(), UsageError> {
        match self.rest.first() {
            Some(extra) => Err(UsageError(format!("Лишний аргумент: {}", extra.to_string_lossy()))),
            None => Ok(()),
        }
    }

    fn remaining(self) -> Vec<String> {
        self.rest.iter().map(|arg| arg.to_string_lossy().into_owned()).collect()
    }
}

impl<'a> Iterator for Words<'a> {
    type Item = Word<'a>;

    fn next(&mut self) -> Option<Word<'a>> {
        let first = self.value()?;
        if self.literal {
            return Some(Word::Positional(first));
        }
        match first.to_str() {
            Some("--") => {
                self.literal = true;
                self.next()
            }
            Some(text) if text.len() > 1 && text.starts_with('-') => Some(Word::Flag(text)),
            _ => Some(Word::Positional(first)),
        }
    }
}

pub(crate) fn parse(args: &[OsString]) -> Result<Command, UsageError> {
    let Some((first, rest)) = args.split_first() else {
        return Ok(Command::Repl);
    };
    match first.to_str() {
        Some("repl") => parse_repl(rest),
        Some("fmt") => parse_fmt(rest),
        Some("ast") => parse_file_command(rest, AST_USAGE, |file| Command::Ast { file }),
        Some("disasm") => parse_file_command(rest, DISASM_USAGE, |file| Command::Disasm { file }),
        Some("lint") => parse_file_command(rest, LINT_USAGE, |file| Command::Lint { file }),
        Some("transpile") => parse_transpile(rest),
        _ => parse_run(args),
    }
}

fn unknown_flag(flag: &str) -> UsageError {
    UsageError(format!("Неизвестный флаг: {flag}"))
}

fn help(usage: &'static str, words: Words<'_>) -> Result<Command, UsageError> {
    words.finish()?;
    Ok(Command::Help(usage))
}

fn set_file(file: &mut Option<PathBuf>, path: &OsStr) -> Result<(), UsageError> {
    if file.is_some() {
        return Err(UsageError(format!("Указан более чем один файл: {}", path.to_string_lossy())));
    }
    *file = Some(PathBuf::from(path));
    Ok(())
}

fn utf8(arg: &OsStr) -> Result<String, UsageError> {
    arg.to_str()
        .map(str::to_string)
        .ok_or_else(|| UsageError(format!("Аргумент не является корректной строкой UTF-8: {}", arg.to_string_lossy())))
}

fn parse_repl(args: &[OsString]) -> Result<Command, UsageError> {
    let mut words = Words::new(args);
    match words.next() {
        None => Ok(Command::Repl),
        Some(Word::Flag("-h" | "--help")) => help(REPL_USAGE, words),
        Some(Word::Flag(extra)) => Err(UsageError(format!("Лишний аргумент: {extra}"))),
        Some(Word::Positional(extra)) => Err(UsageError(format!("Лишний аргумент: {}", extra.to_string_lossy()))),
    }
}

fn parse_file_command(
    args: &[OsString],
    usage: &'static str,
    build: fn(PathBuf) -> Command,
) -> Result<Command, UsageError> {
    let mut file = None;
    let mut words = Words::new(args);
    while let Some(word) = words.next() {
        match word {
            Word::Flag("-h" | "--help") => return help(usage, words),
            Word::Flag(flag) => return Err(unknown_flag(flag)),
            Word::Positional(path) => set_file(&mut file, path)?,
        }
    }
    file.map(build).ok_or_else(|| UsageError(usage.to_string()))
}

fn parse_fmt(args: &[OsString]) -> Result<Command, UsageError> {
    let mut file = None;
    let mut write = false;
    let mut check = false;
    let mut source_map = false;
    let mut words = Words::new(args);
    while let Some(word) = words.next() {
        match word {
            Word::Flag("-h" | "--help") => return help(FMT_USAGE, words),
            Word::Flag("-w" | "--write") => write = true,
            Word::Flag("--check") => check = true,
            Word::Flag("--source-map") => source_map = true,
            Word::Flag(flag) => return Err(unknown_flag(flag)),
            Word::Positional(path) => set_file(&mut file, path)?,
        }
    }
    let file = file.ok_or_else(|| UsageError(FMT_USAGE.to_string()))?;
    let conflict = |message: &str| Err(UsageError(message.to_string()));
    let mode = match (check, write) {
        (true, true) => return conflict("Флаги --check и --write нельзя использовать вместе"),
        (true, false) if source_map => return conflict("Флаги --check и --source-map нельзя использовать вместе"),
        (false, false) if source_map => return conflict("Флаг --source-map требует --write"),
        (true, false) => FmtMode::Check,
        (false, true) => FmtMode::Write,
        (false, false) => FmtMode::Print,
    };
    Ok(Command::Fmt { file, mode, source_map })
}

fn parse_transpile(args: &[OsString]) -> Result<Command, UsageError> {
    let mut file = None;
    let mut output = None;
    let mut words = Words::new(args);
    while let Some(word) = words.next() {
        match word {
            Word::Flag("-h" | "--help") => return help(TRANSPILE_USAGE, words),
            Word::Flag("-o" | "--output") => {
                let path = words.value().ok_or_else(|| UsageError("Флаг -o требует путь к файлу".to_string()))?;
                if output.replace(PathBuf::from(path)).is_some() {
                    return Err(UsageError("Флаг -o указан более одного раза".to_string()));
                }
            }
            Word::Flag(flag) => return Err(unknown_flag(flag)),
            Word::Positional(path) => set_file(&mut file, path)?,
        }
    }
    let file = file.ok_or_else(|| UsageError(TRANSPILE_USAGE.to_string()))?;
    Ok(Command::Transpile { file, output })
}

fn parse_run(args: &[OsString]) -> Result<Command, UsageError> {
    let mut use_vm = false;
    let mut eval = None;
    let mut words = Words::new(args);
    while let Some(word) = words.next() {
        match word {
            Word::Flag("--vm") => use_vm = true,
            Word::Flag("-h" | "--help") => return help(HELP, words),
            Word::Flag("-V" | "--version") => {
                words.finish()?;
                return Ok(Command::Version);
            }
            Word::Flag(flag @ ("-e" | "--eval")) => {
                let code = words.value().ok_or_else(|| UsageError(format!("Флаг {flag} требует аргумент с кодом")))?;
                if eval.replace(utf8(code)?).is_some() {
                    return Err(UsageError("Флаг -e указан более одного раза".to_string()));
                }
            }
            Word::Flag(flag) => return Err(unknown_flag(flag)),
            Word::Positional(first) => {
                let source = match eval.take() {
                    Some(code) if words.literal => {
                        let mut script_args = vec![first.to_string_lossy().into_owned()];
                        script_args.extend(words.remaining());
                        return Ok(Command::Run { source: RunSource::Eval(code), use_vm, script_args });
                    }
                    Some(_) => {
                        return Err(UsageError(
                            "Укажите только один источник программы: файл, -e или - (аргументы для -e идут после --)"
                                .to_string(),
                        ));
                    }
                    None if first == "-" && !words.literal => RunSource::Stdin,
                    None => RunSource::File(PathBuf::from(first)),
                };
                return Ok(Command::Run { source, use_vm, script_args: words.remaining() });
            }
        }
    }
    match eval {
        Some(code) => Ok(Command::Run { source: RunSource::Eval(code), use_vm, script_args: Vec::new() }),
        None => Err(UsageError("Не указан файл для выполнения".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_strs(args: &[&str]) -> Result<Command, UsageError> {
        let args: Vec<OsString> = args.iter().map(OsString::from).collect();
        parse(&args)
    }

    fn usage_message(args: &[&str]) -> String {
        match parse_strs(args) {
            Err(UsageError(message)) => message,
            Ok(command) => panic!("ожидалась ошибка использования, получено {command:?}"),
        }
    }

    fn run(source: RunSource, use_vm: bool, script_args: &[&str]) -> Command {
        Command::Run { source, use_vm, script_args: script_args.iter().map(ToString::to_string).collect() }
    }

    #[test]
    fn no_arguments_start_the_repl() {
        assert_eq!(parse_strs(&[]), Ok(Command::Repl));
    }

    #[test]
    fn repl_subcommand_starts_the_repl() {
        assert_eq!(parse_strs(&["repl"]), Ok(Command::Repl));
    }

    #[test]
    fn repl_rejects_arguments() {
        assert_eq!(usage_message(&["repl", "--vm"]), "Лишний аргумент: --vm");
    }

    #[test]
    fn a_bare_file_runs_on_the_interpreter() {
        assert_eq!(parse_strs(&["a.yopta"]), Ok(run(RunSource::File("a.yopta".into()), false, &[])));
    }

    #[test]
    fn vm_flag_selects_the_vm() {
        assert_eq!(parse_strs(&["--vm", "a.yopta"]), Ok(run(RunSource::File("a.yopta".into()), true, &[])));
    }

    #[test]
    fn everything_after_the_file_belongs_to_the_script() {
        assert_eq!(
            parse_strs(&["a.yopta", "--vm", "-e", "-", "x"]),
            Ok(run(RunSource::File("a.yopta".into()), false, &["--vm", "-e", "-", "x"]))
        );
    }

    #[test]
    fn dash_reads_the_program_from_stdin_and_passes_the_rest_on() {
        assert_eq!(parse_strs(&["-", "x", "--y"]), Ok(run(RunSource::Stdin, false, &["x", "--y"])));
    }

    #[test]
    fn eval_accepts_flags_on_either_side() {
        assert_eq!(parse_strs(&["--eval", "код", "--vm"]), Ok(run(RunSource::Eval("код".into()), true, &[])));
        assert_eq!(parse_strs(&["--vm", "-e", "код"]), Ok(run(RunSource::Eval("код".into()), true, &[])));
    }

    #[test]
    fn eval_takes_script_arguments_after_a_double_dash() {
        assert_eq!(
            parse_strs(&["-e", "код", "--", "x", "--vm"]),
            Ok(run(RunSource::Eval("код".into()), false, &["x", "--vm"]))
        );
    }

    #[test]
    fn eval_code_may_start_with_a_dash() {
        assert_eq!(parse_strs(&["-e", "-1;"]), Ok(run(RunSource::Eval("-1;".into()), false, &[])));
    }

    #[test]
    fn double_dash_introduces_a_file_that_looks_like_a_flag() {
        assert_eq!(parse_strs(&["--", "-f.yopta", "x"]), Ok(run(RunSource::File("-f.yopta".into()), false, &["x"])));
    }

    #[test]
    fn eval_requires_code() {
        assert_eq!(usage_message(&["-e"]), "Флаг -e требует аргумент с кодом");
        assert_eq!(usage_message(&["--eval"]), "Флаг --eval требует аргумент с кодом");
    }

    #[test]
    fn eval_may_be_given_only_once() {
        assert_eq!(usage_message(&["-e", "а", "-e", "б"]), "Флаг -e указан более одного раза");
    }

    #[test]
    fn eval_conflicts_with_a_file_and_with_stdin() {
        let expected = "Укажите только один источник программы: файл, -e или - (аргументы для -e идут после --)";
        assert_eq!(usage_message(&["-e", "код", "a.yopta"]), expected);
        assert_eq!(usage_message(&["-e", "код", "-"]), expected);
    }

    #[test]
    fn flags_alone_do_not_name_a_program() {
        assert_eq!(usage_message(&["--vm"]), "Не указан файл для выполнения");
        assert_eq!(usage_message(&["--"]), "Не указан файл для выполнения");
    }

    #[test]
    fn unknown_flags_are_rejected() {
        assert_eq!(usage_message(&["--nonsense", "a.yopta"]), "Неизвестный флаг: --nonsense");
    }

    #[test]
    fn help_and_version_are_recognised_after_other_flags() {
        assert_eq!(parse_strs(&["--help"]), Ok(Command::Help(HELP)));
        assert_eq!(parse_strs(&["--vm", "-h"]), Ok(Command::Help(HELP)));
        assert_eq!(parse_strs(&["-V"]), Ok(Command::Version));
        assert_eq!(parse_strs(&["--vm", "--version"]), Ok(Command::Version));
    }

    #[test]
    fn help_and_version_reject_trailing_arguments() {
        assert_eq!(usage_message(&["--help", "x"]), "Лишний аргумент: x");
        assert_eq!(usage_message(&["--version", "x"]), "Лишний аргумент: x");
        assert_eq!(usage_message(&["ast", "--help", "x"]), "Лишний аргумент: x");
    }

    #[test]
    fn file_subcommands_take_exactly_one_file() {
        assert_eq!(parse_strs(&["ast", "a.yopta"]), Ok(Command::Ast { file: "a.yopta".into() }));
        assert_eq!(parse_strs(&["disasm", "a.yopta"]), Ok(Command::Disasm { file: "a.yopta".into() }));
        assert_eq!(parse_strs(&["lint", "a.yopta"]), Ok(Command::Lint { file: "a.yopta".into() }));
        assert_eq!(usage_message(&["ast"]), AST_USAGE);
        assert_eq!(usage_message(&["lint", "a.yopta", "b.yopta"]), "Указан более чем один файл: b.yopta");
        assert_eq!(usage_message(&["disasm", "a.yopta", "--x"]), "Неизвестный флаг: --x");
    }

    #[test]
    fn subcommands_print_their_own_usage_on_help() {
        assert_eq!(parse_strs(&["fmt", "--help"]), Ok(Command::Help(FMT_USAGE)));
        assert_eq!(parse_strs(&["ast", "-h"]), Ok(Command::Help(AST_USAGE)));
        assert_eq!(parse_strs(&["disasm", "--help"]), Ok(Command::Help(DISASM_USAGE)));
        assert_eq!(parse_strs(&["lint", "-h"]), Ok(Command::Help(LINT_USAGE)));
        assert_eq!(parse_strs(&["transpile", "--help"]), Ok(Command::Help(TRANSPILE_USAGE)));
        assert_eq!(parse_strs(&["repl", "--help"]), Ok(Command::Help(REPL_USAGE)));
    }

    #[test]
    fn every_usage_line_names_its_subcommand() {
        for (name, usage) in [
            ("fmt", FMT_USAGE),
            ("ast", AST_USAGE),
            ("disasm", DISASM_USAGE),
            ("lint", LINT_USAGE),
            ("transpile", TRANSPILE_USAGE),
            ("repl", REPL_USAGE),
        ] {
            assert!(usage.starts_with(&format!("Использование: yps {name}")), "{usage}");
            assert!(HELP.contains(usage.trim_start_matches("Использование: ")), "справка не упоминает: {usage}");
        }
    }

    #[test]
    fn fmt_accepts_its_file_in_any_position() {
        let expected = Command::Fmt { file: "a.yopta".into(), mode: FmtMode::Check, source_map: false };
        assert_eq!(parse_strs(&["fmt", "--check", "a.yopta"]), Ok(expected));
        let expected = Command::Fmt { file: "a.yopta".into(), mode: FmtMode::Write, source_map: true };
        assert_eq!(parse_strs(&["fmt", "-w", "a.yopta", "--source-map"]), Ok(expected));
        let expected = Command::Fmt { file: "a.yopta".into(), mode: FmtMode::Print, source_map: false };
        assert_eq!(parse_strs(&["fmt", "a.yopta"]), Ok(expected));
    }

    #[test]
    fn fmt_rejects_conflicting_modes() {
        assert_eq!(
            usage_message(&["fmt", "a.yopta", "--check", "--write"]),
            "Флаги --check и --write нельзя использовать вместе"
        );
        assert_eq!(
            usage_message(&["fmt", "--source-map", "a.yopta", "--check"]),
            "Флаги --check и --source-map нельзя использовать вместе"
        );
        assert_eq!(usage_message(&["fmt", "--source-map", "a.yopta"]), "Флаг --source-map требует --write");
    }

    #[test]
    fn fmt_requires_exactly_one_file() {
        assert_eq!(usage_message(&["fmt"]), FMT_USAGE);
        assert_eq!(usage_message(&["fmt", "--check"]), FMT_USAGE);
        assert_eq!(usage_message(&["fmt", "a.yopta", "b.yopta"]), "Указан более чем один файл: b.yopta");
    }

    #[test]
    fn transpile_takes_an_optional_output_path() {
        assert_eq!(
            parse_strs(&["transpile", "a.yopta"]),
            Ok(Command::Transpile { file: "a.yopta".into(), output: None })
        );
        assert_eq!(
            parse_strs(&["transpile", "-o", "a.js", "a.yopta"]),
            Ok(Command::Transpile { file: "a.yopta".into(), output: Some("a.js".into()) })
        );
        assert_eq!(
            parse_strs(&["transpile", "a.yopta", "--output", "a.js"]),
            Ok(Command::Transpile { file: "a.yopta".into(), output: Some("a.js".into()) })
        );
    }

    #[test]
    fn transpile_rejects_a_missing_or_repeated_output() {
        assert_eq!(usage_message(&["transpile", "a.yopta", "-o"]), "Флаг -o требует путь к файлу");
        assert_eq!(
            usage_message(&["transpile", "a.yopta", "-o", "a.js", "-o", "b.js"]),
            "Флаг -o указан более одного раза"
        );
        assert_eq!(usage_message(&["transpile"]), TRANSPILE_USAGE);
    }

    #[cfg(unix)]
    #[test]
    fn a_non_utf8_file_name_is_kept_as_a_path() {
        use std::os::unix::ffi::OsStringExt;

        let name = OsString::from_vec(b"\xff.yopta".to_vec());

        let parsed = parse(std::slice::from_ref(&name));

        assert_eq!(parsed, Ok(run(RunSource::File(PathBuf::from(name)), false, &[])));
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_code_is_a_usage_error() {
        use std::os::unix::ffi::OsStringExt;

        let args = [OsString::from("-e"), OsString::from_vec(b"\xff".to_vec())];

        let Err(UsageError(message)) = parse(&args) else {
            panic!("ожидалась ошибка использования")
        };

        assert!(message.starts_with("Аргумент не является корректной строкой UTF-8"), "{message}");
    }
}
