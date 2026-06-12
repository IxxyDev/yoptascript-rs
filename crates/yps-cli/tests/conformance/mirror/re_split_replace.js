'use strict';
function показ(массив) {
    console.log(массив.length + " :: " + массив.join("|"));
}
показ("a1b2c3".split(/\d/));
показ("a, b,  c,   d".split(/,\s*/));
показ("abc".split(/x/));
показ("2026-06-12".split(/-/));
показ("axbxc".split(/(x)/));
console.log("---replace regex groups---");
console.log("2026-06-12".replace(/(\d+)-(\d+)-(\d+)/, "$3/$2/$1"));
console.log("hello world".replaceAll(/(\w)(\w*)/g, "$2$1"));
console.log("a1b2".replace(/([a-z])(\d)/g, "$2$1"));
