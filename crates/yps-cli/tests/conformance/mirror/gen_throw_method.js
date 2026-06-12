'use strict';
function* сКэтчем() {
    try {
        yield 1;
        yield 2;
    } catch (e) {
        console.log("внутри поймал:", e);
        yield 100;
    }
    yield 3;
}
const г = сКэтчем();
console.log("y1:", г.next().value);
const р = г.throw("бум");
console.log("после throw:", р.value, р.done);
console.log("y3:", г.next().value);
console.log("конец:", г.next().done);

function* безКэтча() {
    yield 1;
    yield 2;
}
const г2 = безКэтча();
console.log("y:", г2.next().value);
try {
    г2.throw("наружу");
} catch (e) {
    console.log("снаружи поймал:", e);
}
console.log("после непойманного:", г2.next().done);

function* наСтарте() {
    yield 1;
}
const г3 = наСтарте();
try {
    г3.throw("до старта");
} catch (e) {
    console.log("на старте поймал:", e);
}

function* завершён() {
    yield 1;
}
const г4 = завершён();
г4.next();
г4.next();
try {
    г4.throw("после конца");
} catch (e) {
    console.log("на завершённом поймал:", e);
}
