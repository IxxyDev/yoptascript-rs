'use strict';
let результат = "нет";
switch (1) {
    case "1":
        результат = "строка1";
        break;
    case 1:
        результат = "число1";
        break;
    default:
        результат = "дефолт";
}
console.log("свитч:", результат);

const мас = [1, 2, 3];
console.log("includes(1):", мас.includes(1));
console.log("includes-строка:", мас.includes("1"));
console.log("indexOf(2):", мас.indexOf(2));
console.log("indexOf-строка:", мас.indexOf("2"));
