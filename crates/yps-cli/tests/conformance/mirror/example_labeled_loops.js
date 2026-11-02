'use strict';
поиск: for (let и = 0; и < 3; и += 1) {
    for (let ж = 0; ж < 3; ж += 1) {
        if (и == 1 && ж == 1) {
            console.log("нашли на " + и + "," + ж);
            break поиск;
        }
    }
}

строки: for (let р = 0; р < 3; р += 1) {
    for (let к = 0; к < 3; к += 1) {
        if (к == 1) {
            continue строки;
        }
        console.log("ячейка " + р + "," + к);
    }
}
