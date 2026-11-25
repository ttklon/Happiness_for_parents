# Happiness Scanner

Полностью автоматический фоновый поиск cryptocurrency-кошельков с балансом на Windows 11.
**Rust + Tauri v2** бэкенд и **React + TypeScript + Tailwind** интерфейс. Наследует идею
[bitgen-by-congrammer](https://github.com/Congrammer/bitgen-by-congrammer), но работает как
настоящее десктопное приложение: трей, автостарт, красивые настройки, бесконечный поиск
без повторов, настраиваемая степень нагрузки и всесторонняя анонимность.

> Это статистический эксперимент: вероятность случайно найти кошелёк с деньгами
> астрономически мала. Не ожидайте гарантированного результата.

## Что реализовано

### Поиск
- **14 монет**: BTC, ETH, LTC, DOGE, BCH, MATIC, BSC, ARB, OPT, BASE, AVAX, TRX, SOL, TON.
- **15 режимов**: `ALL_COINS_AUTO`, `RANDOM`, `BRAIN_WALLET`, `BRAIN_COMBOS`,
  `MULTI_WORD_COMBO`, `BIP39_PASSPHRASE`, `TARGET_ADDRESSES`, `MULTI_COIN_BRAIN`,
  `TRANSACTION_HISTORY`, `BLOOM_FILTER`, `VANITY_PATTERN`, `SEQUENTIAL_KEYS`,
  `RICH_LIST`, `PUZZLE`, `DICTIONARY`. `ALL_COINS_AUTO` сам циклически перебирает
  все монеты × все режимы и работает бесконечно.
- **Адреса**: P2PKH, P2SH-P2WPKH, P2WPKH (bech32), P2TR (bech32m/taproot),
  EVM `0x…` (keccak256), TRON (base58check 0x41), Solana (ed25519 base58), TON (EQ…).
- **8 языков BIP39** с полноценными списками по 2048 слов (английский, испанский,
  французский, итальянский, японский, корейский, упрощённый и традиционный китайский).
- **BIP32/BIP44/49/84/86** деривация (`m/purpose'/coin'/0'/0/0`), PBKDF2-HMAC-SHA512.
- **Мутации brain-паролей**: leet, регистр, переворот, удаление гласных, года,
  суффиксы, повторы, комбинации из 2–6 слов.
- **Никаких повторов**: каждый кандидат имеет отпечаток в `HashSet`; повторная проверка
  невозможна, плюс TTL-кэш адресов (по умолчанию 60 минут).
- **Максимум шансов**: режим `derive_all_paths` проверяет BIP44/49/84/86 для каждой фразы
  (в 4 раза больше адресов), режим `TARGET_ADDRESSES` берёт адреса из локального файла.
- **Загрузка**: 1–32 воркера, размер батча 1–500, задержка между батчами.

### Сеть
- Бесплатные публичные API с **автоматическим failover** и отслеживанием здоровья
  endpoints: blockstream.info, mempool.space, blockchain.info, Blockchair, BlockCypher,
  Trongrid, Solana RPC, Toncenter, Ankr/Blockscout для EVM.
- Endpoint с 3+ ошибками уходит в карантин с экспоненциальным временем восстановления.
- Поддержка HTTP-прокси с авторизацией и SOCKS5/Tor.

### Автоматические обновления
- Приложение **раз в 60 секунд** опрашивает GitHub Releases и показывает уведомление о новой версии.
- Кнопка «Скачать и установить» скачивает установщик, проверяет **SHA-256** из релиза (`checksums.txt`) и запускает его.
- При обновлении **ничего не сбрасывается**: `config.json`, hits и empties лежат в `%APPDATA%`, а не в папке программы.
- Проверку можно выключить в настройках (`update_check_enabled`).

### Анонимность — «одна кнопка, всё само»
Мастер-переключатель `full_anonymity_mode` включён по умолчанию и при старте применяет:

| Защита | Что делает |
|---|---|
| Tor / SOCKS5 | трафик API через `socks5h://127.0.0.1:9050` (DNS резолвится на прокси — без утечки) |
| Случайный User-Agent | уникальный UA для каждого запроса |
| Случайные заголовки | Accept-Language, Accept, DNT — ротируются |
| Ротация прокси | смена между запросами |
| Stealth + маска окна | окно называется `System Host Process` |
| AES-256-GCM | шифрование hits/empty (PBKDF2-SHA256, 100 000 итераций) |
| Только в памяти | на диск не пишется вообще ничего |
| Килл-свитч | стоп после N ошибок API |
| Блок direct HTTP | без прокси запрос вообще не уходит |
| Без телеметрии | только локальное хранилище |
| Ротация прокси | пул прокси из настроек, новый прокси на запрос |
| Jitter | случайная добавка к задержке между батчами |
| Приватность лога | фразы/пароли не попадают в журнал |

Пароль шифрования можно сгенерировать автоматически в настройках.

### Интерфейс
- Дашборд: счётчики, скорость, uptime, спарклайн, живой лог и карточка последней проверки.
- Настройки: нагрузка, монеты/режимы, кэш, все слои анонимности, уведомления,
  автозапуск, экспорт/импорт конфига, проверка API.
- Найденные: поиск, фильтр по монете, экспорт CSV/JSON, тосты о новых находках.
- Диагностика: здоровье API, статус защит, лог.
- Окно закрывается в трей; сканирование продолжается в фоне.

## Структура

```
src-tauri/
  Cargo.toml
  tauri.conf.json
  capabilities/default.json
  icons/icon.png, icon.ico
  src/
    main.rs        Tauri-команды, трей, автозапуск
    config.rs      настройки, валидация, пути %APPDATA%
    state.rs       общее состояние, статистика, лог
    wallet.rs      BIP39/BIP32, адреса 14 монет, мутации
    balance.rs     бесплатные API, failover, здоровье endpoints
    scanner.rs     бесконечный движок воркеров, дедупликация, килл-свитч
    results.rs     JSONL, шифрование результатов
    crypto.rs      AES-256-GCM + PBKDF2
    notifier.rs    Telegram + Discord embed
    bip39_*.txt    словари 8 языков
frontend/
  src/{App.tsx,pages,components,lib}  React UI
  src/input.css                       Tailwind
  dist/                               сборка (Ignored by git)
.github/workflows/build.yml           CI: cargo check, cargo test, MSI
```

## Сборка

### Локально на Windows 11
```powershell
cd frontend
npm install
npm run build
cd ..\src-tauri
cargo tauri build
```
Готовый MSI: `src-tauri/target/release/bundle/msi/`.

Требования: [Rust stable](https://rustup.rs/), [Tauri CLI v2](https://v2.tauri.app/),
Node.js 22+, Visual Studio Build Tools (C++ desktop) и WebView2.

### GitHub Actions
Workflow `.github/workflows/build.yml` при каждом push на Windows runner:
устанавливает Node и Rust, собирает фронтенд, выполняет `cargo check`, юнит-тесты
криптографии (`cargo test`) и собирает установщики MSI + NSIS. При push тега `v*`
создаётся публичный GitHub Release с установщиками и файлом `checksums.txt` —
именно оттуда приложение берёт обновления.

### Веб-превью интерфейса
```bash
cd frontend
npm install
npm run dev
```
В браузере включается демонстрационный режим (данные симулируются), чтобы можно было
оценить интерфейс без запуска Rust-бэкенда.

## Где лежат данные

- `%APPDATA%\Happiness Scanner\config.json`
- `%APPDATA%\Happiness Scanner\results\hits.jsonl`
- `%APPDATA%\Happiness Scanner\results\empty.jsonl`

При включённом `memory_only_mode` файлы результатов не создаются.

## Тесты
`cargo test` в `src-tauri` проверяет известные векторы BIP39, BIP32 и Base58Check —
это защищает от ошибок в генерации адресов.

## Disclaimer
Инструмент предназначен для обучения и экспериментов. Проверяйте только адреса,
которые вы сами вывели. Не нарушайте правила использования публичных API.
