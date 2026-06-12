'use strict';
async function main() {
    const итог = await Promise.resolve(1)
        .then((v) => v + 10)
        .then((v) => v * 2);
    console.log("цепочка:", итог);

    const вложен = await Promise.resolve(5)
        .then((v) => Promise.resolve(v * 100));
    console.log("вложенный потом:", вложен);

    try {
        await Promise.resolve(1)
            .then(() => { throw "ошибка в потом"; })
            .then(() => console.log("этот потом пропущен"));
    } catch (e) {
        console.log("поймано из цепочки:", e);
    }

    const черезЛовить = await Promise.reject("боль")
        .catch((e) => "восстановлено: " + e);
    console.log(черезЛовить);
}
main();
