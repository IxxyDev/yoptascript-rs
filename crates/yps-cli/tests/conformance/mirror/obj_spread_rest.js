'use strict';
const сл1 = { а: 1, б: 2 };
const сл2 = { б: 99, в: 3 };
const объед = { ...сл1, ...сл2 };
console.log(объед.а);
console.log(объед.б);
console.log(объед.в);

const м1 = [1, 2];
const м2 = [0, ...м1, 3];
console.log(м2.length);
console.log(м2[0]);
console.log(м2[1]);
console.log(м2[2]);
console.log(м2[3]);

let { имя, ...прочее } = { имя: 'Ян', возраст: 30, город: 'М' };
console.log(имя);
console.log(прочее.возраст);
console.log(прочее.город);

let [пер, ...хвост] = [10, 20, 30];
console.log(пер);
console.log(хвост.length);
console.log(хвост[0]);

function сумм(...нс) { let с = 0; for (const н of нс) { с += н; } return с; }
const арги = [1, 2, 3, 4];
console.log(сумм(...арги));
