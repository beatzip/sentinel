# 📁 Структура проекта

## 🎯 Как использовать

### 1. Подготовка карт
```cmd
scripts\convert_all_navs.bat
```
Это:
- Скачает актуальные .nav файлы через awpy
- Конвертирует их в JSON формат
- Сохранит в `crates/sentinel-map/assets/nav/`

### 2. Анализ демо файлов
Положите демо файлы в папку `demo/` и запустите:
```cmd
analyze_demos.bat
```

Результаты будут в тех же файлах:
- `demo/match.json` - JSON отчёт
- `demo/match.html` - HTML отчёт

### 3. Папки

```
sentinel/
├── demo/              # Ваші демо файли
├── scripts/           # Скрипти для конверсії
├── crates/            # Rust код
│   └── sentinel-map/
│       └── assets/
│           └── nav/   # Навігаційні файли JSON
└── tris/              # .tri файли карт
```

### 4. Підтримувані карти

Наразі налаштовано:
- ✅ `de_dust2` (існуючі файли)
- ✅ `de_mirage` (потрібно скачати .tri)
- ✅ `de_inferno` (потрібно скачати .tri)
- ✅ Інші карти CS2

### 5. Додаткова інформація

Для повного оновлення карт:
```cmd
python -c "from awpy import get_tris, get_nav; get_tris(); get_nav()"
```

Потім запустіть:
```cmd
scripts\convert_all_navs.bat
```