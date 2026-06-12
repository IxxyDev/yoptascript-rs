'use strict';
function* простой() {
    yield 1;
    yield 2;
    yield 3;
}
const г = простой();
console.log("y1:", г.next().value);
const р = г.return(42);
console.log("ret:", р.value, р.done);
console.log("после:", г.next().value, г.next().done);

function* сФиналли() {
    try {
        yield "a";
        yield "b";
    } finally {
        console.log("финалли выполнен");
    }
}
const г2 = сФиналли();
console.log("y:", г2.next().value);
const р2 = г2.return("стоп");
console.log("ret2:", р2.value, р2.done);

function* финаллиОверрайд() {
    try {
        yield "x";
    } finally {
        return "из-финалли";
    }
}
const г3 = финаллиОверрайд();
console.log("y3:", г3.next().value);
const р3 = г3.return("игнор");
console.log("ret3:", р3.value, р3.done);
