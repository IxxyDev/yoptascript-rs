'use strict';
async function main() {
    const а = await Promise.resolve("значение")
        .finally(() => console.log("наконец 1 выполнен"));
    console.log("после наконец:", а);

    const б = await Promise.resolve("исходное")
        .finally(() => "это игнорируется");
    console.log("наконец не меняет значение:", б);

    try {
        await Promise.resolve("ок")
            .finally(() => { throw "из наконец"; });
    } catch (e) {
        console.log("наконец заменяет throw-ом:", e);
    }
}
main();
