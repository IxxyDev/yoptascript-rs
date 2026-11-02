'use strict';
const ПИ = 3.14159;
function удвоить(х) {
    return х * 2;
}
function приватная() {
    return "не должна экспортироваться";
}
let переменная = 100;
module.exports = { ПИ, удвоить, переменная };
