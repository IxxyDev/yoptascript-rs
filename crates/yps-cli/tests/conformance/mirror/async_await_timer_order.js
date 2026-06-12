// DIVERGENCE: KNOWN_DIVERGENCES.md §8.2 синхронный await не возвращает управление вызывающему
'use strict';
function задержка(значение, мс) {
    return new Promise((решить, _) => {
        setTimeout(() => решить(значение), мс);
    });
}

async function main() {
    console.log("старт");
    setTimeout(() => console.log("таймер 0мс"), 0);
    Promise.resolve().then(() => console.log("микротаска до таймера"));
    const значение = await задержка("после таймера", 10);
    console.log(значение);
    setTimeout(() => console.log("таймер в конце"), 5);
    console.log("конец main");
}
main();
console.log("после main");
