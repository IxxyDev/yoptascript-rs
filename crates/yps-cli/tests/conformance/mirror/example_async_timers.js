// DIVERGENCE: СигналОтмены.отВремени rejects with a Russian-text error (message "Тайм-аут"); Node's AbortSignal.timeout rejects with a TimeoutError whose message is English
'use strict';
function подождать(мс, опции) {
    return new Promise((решить, отвергнуть) => {
        const сигнал = опции && опции.сигнал;
        const т = setTimeout(решить, мс);
        if (сигнал) {
            сигнал.addEventListener('abort', () => {
                clearTimeout(т);
                отвергнуть(сигнал.reason);
            });
        }
    });
}

async function демо() {
    await подождать(30);
    console.log("после подождать(30)");

    const контроллёр = new AbortController();
    setTimeout(() => контроллёр.abort("стоп"), 10);
    try {
        await подождать(200, { сигнал: контроллёр.signal });
        console.log("не должно сюда дойти");
    } catch (e) {
        console.log("ожидание отменено:", e.message);
    }

    const итог = await Promise.race([
        подождать(50),
        new Promise((решить, _) => { setTimeout(() => решить("быстрее"), 5); })
    ]);
    console.log("гонка дала:", итог);

    const сигналТо = AbortSignal.timeout(10);
    const обещание = new Promise((_, отвергнуть) => {
        сигналТо.addEventListener('abort', () => отвергнуть(сигналТо.reason));
    });
    try {
        await обещание;
    } catch (e) {
        console.log("отВремени:", e.message);
    }
}

демо();
