'use strict';
let { а = 5 } = { а: null };
console.log(а);

let { б = 5 } = { б: undefined };
console.log(б);

let { в = 5 } = { в: 0 };
console.log(в);

let { г: псевдо = 'по умолчанию' } = {};
console.log(псевдо);

let { г: псевдо2 = 'по умолчанию' } = { г: 'значение' };
console.log(псевдо2);

let [п, , т] = [1, 2, 3];
console.log(п);
console.log(т);

let [х = 10, у = 20] = [1];
console.log(х);
console.log(у);

let { внешний: { внутренний } } = { внешний: { внутренний: 77 } };
console.log(внутренний);

function тест(имя, счёт) {
  console.log(имя);
  console.log(счёт);
}
тест('йопта', 5);
тест('мир', 0);
