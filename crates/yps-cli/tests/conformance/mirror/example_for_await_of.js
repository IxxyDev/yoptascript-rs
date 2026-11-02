'use strict';
async function собрать() {
    const обещания = [
        Promise.resolve(1),
        Promise.resolve(2),
        Promise.resolve(3),
        Promise.resolve(4)
    ];
    let сумма = 0;
    for await (const х of обещания) {
        console.log("получил", х);
        сумма += х;
    }
    return сумма;
}

собрать().then((итог) => {
    console.log("итог:", итог);
});
