'use strict';
console.log(приветствие("Мир"));

function приветствие(имя) {
    return "Привет, " + имя + "!";
}

console.log(чётное(10));

function чётное(н) {
    if (н == 0) { return true; }
    return нечётное(н - 1);
}

function нечётное(н) {
    if (н == 0) { return false; }
    return чётное(н - 1);
}
