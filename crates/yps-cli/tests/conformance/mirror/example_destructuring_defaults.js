'use strict';
const { ширина = 100, высота = 50 } = { ширина: 800 };
console.log(ширина);
console.log(высота);

const { цвет: оттенок = "чёрный" } = {};
console.log(оттенок);

const { сторона = 4, площадь = сторона * сторона } = { сторона: 5 };
console.log(площадь);

const [первый, второй = 2, третий = 3] = [10];
console.log(первый);
console.log(второй);
console.log(третий);
