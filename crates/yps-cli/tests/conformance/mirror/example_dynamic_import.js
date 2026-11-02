'use strict';
async function главная() {
    const мод = await Promise.resolve(require("./modules/математика"));
    console.log("ПИ из модуля =", мод.ПИ);
    console.log("удвоить(21) =", мод.удвоить(21));
    console.log("переменная =", мод.переменная);
}

главная();
