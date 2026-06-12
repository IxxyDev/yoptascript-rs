'use strict';
var c = 0;
[10, 20, 30].some((el, i, arr) => {
    c++;
    console.log(el + ':' + i + ':' + arr.length);
    return el > 15;
});
console.log(c);
c = 0;
[10, 20, 30].every((el, i, arr) => {
    c++;
    console.log(el + ':' + i + ':' + arr.length);
    return el < 25;
});
console.log(c);
c = 0;
[10, 20, 30].find((el, i, arr) => {
    c++;
    console.log(el + ':' + i + ':' + arr.length);
    return el > 15;
});
console.log(c);
c = 0;
[10, 20, 30].findIndex((el, i, arr) => {
    c++;
    console.log(el + ':' + i + ':' + arr.length);
    return el > 15;
});
console.log(c);
c = 0;
[10, 20, 30].findLast((el, i, arr) => {
    c++;
    console.log(el + ':' + i + ':' + arr.length);
    return el < 25;
});
console.log(c);
c = 0;
[10, 20, 30].findLastIndex((el, i, arr) => {
    c++;
    console.log(el + ':' + i + ':' + arr.length);
    return el < 25;
});
console.log(c);
