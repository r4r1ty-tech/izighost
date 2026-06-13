// Заглушка для D-Bus общения
export class IziGhostDBus {
    constructor(extension) {
        this._extension = extension;
        console.log('IziGhost: D-Bus инициализирован (заглушка)');
    }

    sendChatMessage(text) {
        console.log('IziGhost D-Bus stub: отправка сообщения:', text);
    }

    triggerOcr(pngBytes) {
        console.log('IziGhost D-Bus stub: триггер OCR');
    }

    startListening() {
        console.log('IziGhost D-Bus stub: старт аудио');
    }

    stopListening() {
        console.log('IziGhost D-Bus stub: стоп аудио');
    }

    destroy() {
        console.log('IziGhost: D-Bus уничтожен');
    }
}
