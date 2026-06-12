'use strict';
function* вн() {
    const а = yield "вн1";
    console.log("вн получил:", а);
    const б = yield "вн2";
    console.log("вн получил:", б);
    return "вн-конец";
}
function* внеш() {
    yield "до";
    yield* вн();
    yield "после";
}
const г = внеш();
console.log(г.next().value);
console.log(г.next("S0").value);
console.log(г.next("S1").value);
console.log(г.next("S2").value);
console.log(г.next().done);

function* внФиналли() {
    try {
        yield "i1";
        yield "i2";
    } finally {
        console.log("вн финалли");
    }
}
function* внешРет() {
    yield* внФиналли();
    yield "недостижимо";
}
const г2 = внешРет();
console.log(г2.next().value);
const р = г2.return("СТОП");
console.log("delegate ret:", р.value, р.done);

function* внКэтч() {
    try {
        yield "j1";
        yield "j2";
    } catch (e) {
        console.log("вн поймал:", e);
        yield "восстановлен";
    }
}
function* внешThrow() {
    yield* внКэтч();
    yield "хвост";
}
const г3 = внешThrow();
console.log(г3.next().value);
const р3 = г3.throw("БАХ");
console.log("delegate throw:", р3.value, р3.done);
console.log(г3.next().value);
