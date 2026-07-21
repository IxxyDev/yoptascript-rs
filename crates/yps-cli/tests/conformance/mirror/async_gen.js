'use strict';
async function удвоить(н) {
    return н * 2;
}

async function* числа() {
    yield await удвоить(1);
    yield await удвоить(2);
    yield* [10, 20];
}

async function главная() {
    for await (const х of числа()) {
        console.log(х);
    }
    console.log("готово");
}

главная();
