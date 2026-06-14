import os
import sys

# Пытаемся импортировать faster-whisper
try:
    from faster_whisper import WhisperModel
except ImportError:
    print("Ошибка: Зависимость 'faster-whisper' не найдена в системе.", file=sys.stderr)
    print("Пожалуйста, установите её перед запуском (например, через 'pip install faster-whisper' или с помощью системного пакетного менеджера).", file=sys.stderr)
    sys.exit(2)

def main():
    if len(sys.argv) < 2:
        print("Использование: python3 asr_fallback.py <путь_к_wav_файлу>", file=sys.stderr)
        sys.exit(1)
        
    wav_path = sys.argv[1]
    if not os.path.exists(wav_path):
        print(f"Файл {wav_path} не найден", file=sys.stderr)
        sys.exit(1)
        
    # Загружаем компактную модель tiny (мультиязычная, поддерживает RU и EN, ~75MB)
    # Запуск на CPU с квантованием int8 для максимальной легковесности и скорости
    model = WhisperModel("tiny", device="cpu", compute_type="int8")
    
    segments, info = model.transcribe(wav_path, beam_size=5)
    
    text = "".join(segment.text for segment in segments)
    print(text.strip())

if __name__ == '__main__':
    main()
