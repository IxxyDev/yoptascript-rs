#!/usr/bin/env node
'use strict';

const ob = { valueOf() { return 42; } };
console.log('valueOf+0:', ob + 0);
console.log('valueOf==42:', ob == 42);

const str = { toString() { return 'привет'; } };
console.log('toString-concat:', '' + str);

const oba = {
    valueOf() { return 7; },
    toString() { return 'семь'; }
};
console.log('default-valueOf:', oba + 1);

const sprim = {
    [Symbol.toPrimitive](hint) { return 100; },
    valueOf() { return 1; }
};
console.log('вПримитив-приоритет:', sprim + 0);
