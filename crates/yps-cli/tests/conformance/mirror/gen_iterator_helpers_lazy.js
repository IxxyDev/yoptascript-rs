'use strict';
function* источник() {
    let и = 0;
    while (true) {
        console.log("выдаю", и);
        yield и;
        и = и + 1;
    }
}
const рез = Iterator.from(источник())
    .map((н) => { console.log("map", н); return н * 10; })
    .filter((н) => { console.log("filter", н); return н % 20 == 0; })
    .take(2)
    .toArray();
console.log("длина:", рез.length);
console.log(рез[0]);
console.log(рез[1]);

const числа = [1, 2, 3, 4, 5, 6, 7, 8];
const первыеТри = Iterator.from(числа).drop(2).take(3).toArray();
console.log("drop+take длина:", первыеТри.length);
console.log(первыеТри[0]);
console.log(первыеТри[1]);
console.log(первыеТри[2]);
