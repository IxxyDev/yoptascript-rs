'use strict';
function задержка(значение, мс) {
    return new Promise((решить, _) => {
        setTimeout(() => решить(значение), мс);
    });
}

async function main() {
    const упорядочено = await Promise.all([
        задержка("a", 15),
        задержка("b", 5),
        задержка("c", 10)
    ]);
    console.log("всех порядок:", упорядочено[0], упорядочено[1], упорядочено[2]);

    try {
        await Promise.all([
            Promise.resolve(1),
            Promise.reject("первая боль"),
            Promise.resolve(3)
        ]);
    } catch (e) {
        console.log("всех отклонён:", e);
    }

    const устакан = await Promise.allSettled([
        Promise.resolve("ок1"),
        Promise.reject("плохо"),
        Promise.resolve("ок2")
    ]);
    for (const э of устакан) {
        if (э.status === "fulfilled") {
            console.log("выполнено", "значение=" + э.value);
        } else {
            console.log("отклонено", "причина=" + э.reason);
        }
    }
}
main();
