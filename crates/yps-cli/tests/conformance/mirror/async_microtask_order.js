'use strict';
async function main() {
    const p = Promise.resolve(0);
    console.log(1);
    p.then(() => console.log(2));
    p.then(() => console.log(3));
    console.log(4);
    await p;
}
main();
