'use strict';
async function main() {
    const p = Promise.resolve(0);
    console.log("старт");
    p.then(() => {
        console.log("внешний A");
        Promise.resolve().then(() => console.log("вложенный A"));
    });
    p.then(() => {
        console.log("внешний B");
        Promise.resolve().then(() => console.log("вложенный B"));
    });
    console.log("конец синхронного");
    await Promise.resolve()
        .then(() => null)
        .then(() => null)
        .then(() => null);
}
main();
