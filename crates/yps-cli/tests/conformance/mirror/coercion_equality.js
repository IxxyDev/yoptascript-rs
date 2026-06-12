#!/usr/bin/env node
'use strict';

console.log(String(1 == '1'));
console.log(String(1 === '1'));
console.log(String(null == undefined));
console.log(String(null === undefined));
console.log(String(true == 1));
console.log(String(0 == false));
console.log(String('' == 0));
console.log(String(undefined == 0));
console.log(String(null == 0));
console.log(String('abc' == 'abc'));
console.log(String(1 != '1'));
console.log(String(1 !== '1'));
