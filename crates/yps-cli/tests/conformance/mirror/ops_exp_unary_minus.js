// DIVERGENCE: KNOWN_DIVERGENCES.md §4.1 unary minus binds tighter than ** (yopta: (-2)**2; JS: SyntaxError, precedence would be -(2**2))
'use strict';

console.log(-(2 ** 2));
console.log(-(3 ** 2));
console.log(-(2 ** 3));
