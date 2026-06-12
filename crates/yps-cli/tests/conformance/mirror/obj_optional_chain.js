'use strict';
const а = null;
console.log(а?.length);
console.log(а?.б?.в);

const об = { б: { в: 7 } };
console.log(об?.б?.в);
console.log(об?.нету?.ещё);

const ф = null;
console.log(ф?.());

const об2 = { привет() { return 'хай'; } };
console.log(об2.привет?.());
console.log(об2.нету?.());

let счёт = 0;
function поб() { счёт++; return { м() { return 9; } }; }
console.log(null?.[поб()]);
console.log(счёт);

const мас = [10, 20, 30];
console.log(мас?.[1]);
const нет = null;
console.log(нет?.[0]);
