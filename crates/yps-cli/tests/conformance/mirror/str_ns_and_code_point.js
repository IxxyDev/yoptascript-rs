'use strict';
console.log(String.fromCharCode(72, 105));
console.log(String.fromCharCode(72, 105));
console.log(String.fromCodePoint(128512));
console.log(String.fromCodePoint(97, 98));

function tag(strings, ...values) {
    let r = '';
    for (let i = 0; i < strings.length; i += 1) {
        r += strings.raw[i];
        if (i < values.length) {
            r += '<' + values[i] + '>';
        }
    }
    return r;
}
const imya = 'Мир';
console.log(tag`Привет, ${imya}!\n`);

console.log('abc'.codePointAt(0));
console.log('abc'.codePointAt(1));
console.log('😀'.codePointAt(0));
console.log('x'.codePointAt(5));
