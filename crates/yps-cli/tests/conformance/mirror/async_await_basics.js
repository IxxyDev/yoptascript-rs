'use strict';
async function возвращает(значение) {
    return значение;
}
async function бросает() {
    throw "ошибка внутри async";
}
async function ждётОтклонение() {
    try {
        await Promise.reject("отклонённое обещание");
    } catch (e) {
        return "поймано: " + e;
    }
}

async function main() {
    const значение = await возвращает(123);
    console.log("async вернул:", значение);

    const безОбещания = await 42;
    console.log("await не-обещания:", безОбещания);

    try {
        await бросает();
    } catch (e) {
        console.log("throw в async →", e);
    }

    const результат = await ждётОтклонение();
    console.log(результат);

    const цепь = возвращает("через потом");
    цепь.then((v) => console.log("async как обещание:", v));
}
main();
