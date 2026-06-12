'use strict';
function* диалог() {
    const а = yield "первый";
    console.log("получил a:", а);
    const б = yield "второй";
    console.log("получил b:", б);
    return а + б;
}
const г = диалог();
const р1 = г.next("игнор");
console.log("y1:", р1.value, р1.done);
const р2 = г.next(10);
console.log("y2:", р2.value, р2.done);
const р3 = г.next(20);
console.log("ret:", р3.value, р3.done);
const р4 = г.next(99);
console.log("после:", р4.value, р4.done);
