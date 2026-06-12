'use strict';
function* счёт() {
    try {
        let и = 0;
        while (true) {
            yield и;
            и = и + 1;
        }
    } finally {
        console.log("генератор закрыт");
    }
}
let сумма = 0;
for (const х of счёт()) {
    if (х >= 3) {
        break;
    }
    сумма = сумма + х;
}
console.log("сумма:", сумма);

const собрано = [];
for (const х of счёт()) {
    собрано.push(х);
    if (собрано.length >= 2) {
        break;
    }
}
console.log("собрано:", собрано[0], собрано[1]);
