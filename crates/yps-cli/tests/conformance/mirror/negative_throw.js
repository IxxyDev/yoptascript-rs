// DIVERGENCE: an uncaught throw is reported by the interpreter as a Russian diagnostic on stderr plus exit code 1; the harness compares only stdout, so the mirror catches the throw and prints the golden's report format itself (node would print its own English trace to stderr and exit 1)
'use strict';
try {
    throw "проверка";
} catch (e) {
    console.log("---STDERR---");
    console.log(`<КЕЙС>:1:1: Ошибка: Необработанное исключение: ${e}`);
    console.log("---EXIT:1---");
}
