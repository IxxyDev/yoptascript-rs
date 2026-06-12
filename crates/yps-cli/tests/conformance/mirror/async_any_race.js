'use strict';
function задержка(значение, мс) {
    return new Promise((решить, _) => {
        setTimeout(() => решить(значение), мс);
    });
}
function отказЧерез(причина, мс) {
    return new Promise((_, отвергнуть) => {
        setTimeout(() => отвергнуть(причина), мс);
    });
}

async function main() {
    const первый = await Promise.race([
        задержка("медленный", 20),
        задержка("быстрый", 5)
    ]);
    console.log("гонка fulfill:", первый);

    try {
        await Promise.race([
            отказЧерез("ранний отказ", 5),
            задержка("поздний успех", 20)
        ]);
    } catch (e) {
        console.log("гонка reject:", e);
    }

    const выбор = await Promise.any([
        отказЧерез("боль1", 5),
        задержка("успех", 10),
        отказЧерез("боль2", 2)
    ]);
    console.log("любой:", выбор);

    try {
        await Promise.any([
            Promise.reject("e1"),
            Promise.reject("e2")
        ]);
    } catch (e) {
        console.log("любой все отклонены, errors длина:", e.errors.length);
        console.log(e.errors[0]);
        console.log(e.errors[1]);
    }
}
main();
