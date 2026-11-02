'use strict';
let флаг = true;
console.log("флаг:", флаг);
console.log("!флаг:", !флаг);
console.log("ноль:", null);

if (флаг) {
    console.log("флаг - правда");
}

if (false) {
    console.log("это не выведется");
} else {
    console.log("лож - это лож");
}

const ПИ = 3.14;
console.log("ПИ:", ПИ);

let х = 10;
х += 5;
console.log("х += 5:", х);
х -= 3;
console.log("х -= 3:", х);
х *= 2;
console.log("х *= 2:", х);
х /= 4;
console.log("х /= 4:", х);

let и = 0;
while (и < 5) {
    console.log("и =", и);
    и++;
}

let к = 3;
к--;
console.log("к после --:", к);

for (let н = 0; н < 3; н++) {
    console.log("н =", н);
}

function факториал(н) {
    if (н <= 1) {
        return 1;
    }
    return н * факториал(н - 1);
}
console.log("факториал(5) =", факториал(5));
