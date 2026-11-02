'use strict';
process.on('uncaughtException', (e) => console.error(e));

function задержка(значение, мс) {
    return new Promise((решить, _) => {
        setTimeout(() => решить(значение), мс);
    });
}

async function собрать() {
    const а = await задержка(1, 20);
    const б = await задержка(2, 20);
    const в = await задержка(3, 20);
    return а + б + в;
}

function рискованно() {
    return new Promise((_, отвергнуть) => {
        setTimeout(() => отвергнуть("плохо вышло"), 5);
    });
}
async function поймать() {
    try {
        await рискованно();
    } catch (e) {
        console.log("поймано:", e);
    }
}

async function main() {
    console.log("=== Таймеры ===");

    setTimeout(() => console.log("задержка 30мс"), 30);
    setTimeout(() => console.log("задержка 5мс"), 5);
    queueMicrotask(() => console.log("микротаска"));

    console.log("конец синхронного блока");

    await new Promise((решить, _) => setTimeout(() => решить(null), 40));

    console.log("=== Интервал ===");

    let счёт = 0;
    let ид = null;
    await new Promise((решить, _) => {
        ид = setInterval(() => {
            счёт = счёт + 1;
            console.log("тик", счёт);
            if (счёт >= 3) {
                clearInterval(ид);
                console.log("интервал остановлен");
                решить(null);
            }
        }, 10);
    });

    console.log("=== Async/await с задержкой ===");

    await собрать().then((итог) => console.log("сумма:", итог));

    console.log("=== Top-level await ===");
    const х = await задержка("привет из await", 5);
    console.log(х);

    console.log("=== Reject через await + try/catch ===");
    await поймать();

    console.log("=== Throw в timer не убивает loop ===");
    setTimeout(() => { throw "ошибка в таймере"; }, 1);
    setTimeout(() => console.log("следующий таймер всё ещё работает"), 5);
}

main();
