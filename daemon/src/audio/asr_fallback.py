import os
import sys
import subprocess

def setup_venv_and_reexec():
    venv_dir = os.path.expanduser("~/.cache/izighost/venv")
    venv_python = os.path.join(venv_dir, "bin", "python3")
    
    if not os.path.exists(venv_python):
        print("Создание виртуального окружения Python...", file=sys.stderr)
        os.makedirs(os.path.dirname(venv_dir), exist_ok=True)
        subprocess.run([sys.executable, "-m", "venv", venv_dir], check=True)
        
        print("Установка faster-whisper в виртуальное окружение...", file=sys.stderr)
        subprocess.run([os.path.join(venv_dir, "bin", "pip"), "install", "--upgrade", "pip"], check=True)
        subprocess.run([os.path.join(venv_dir, "bin", "pip"), "install", "faster-whisper"], check=True)
    else:
        # Проверяем корректность установки
        try:
            subprocess.run([venv_python, "-c", "import faster_whisper"], check=True, capture_output=True)
        except subprocess.CalledProcessError:
            print("Установка отсутствующей библиотеки faster-whisper...", file=sys.stderr)
            subprocess.run([os.path.join(venv_dir, "bin", "pip"), "install", "faster-whisper"], check=True)

    # Перезапускаем скрипт внутри venv
    os.execv(venv_python, [venv_python] + sys.argv)

# Пытаемся импортировать faster-whisper
try:
    from faster_whisper import WhisperModel
except ImportError:
    setup_venv_and_reexec()
    from faster_whisper import WhisperModel

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
