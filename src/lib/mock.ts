import type { Project, User } from "./types";

export const USER: User = { name: "Артём", handle: "@artyom", initials: "АК" };

export const MOCK_PROJECTS: Project[] = [
  {
    id: "aurora",
    name: "Aurora API",
    emoji: "🚀",
    color: "#7c7dff",
    pinned: true,
    status: "Активен",
    tags: ["rust", "axum", "postgres"],
    path: "~/dev/aurora-api",
    desc: "Бэкенд платёжного шлюза на Rust (Axum + SQLx). Обрабатывает вебхуки, идемпотентные транзакции и сверку с банком. Сейчас готовим релиз v0.9 с новой схемой ретраев.",
    git: { branch: "feature/idempotency", ahead: 2, behind: 0, dirty: 3, lastHash: "a1f4e2c", lastMsg: "refactor: вынес ретраи в отдельный воркер", staged: 1, untracked: 2 },
    commands: [
      { label: "Запустить dev", run: "cargo watch -x run", icon: "play", tint: "#7c7dff" },
      { label: "Тесты", run: "cargo test --all", icon: "flask-conical", tint: "#3fb863" },
      { label: "Миграции", run: "sqlx migrate run", icon: "database", tint: "#5b9cff" },
      { label: "Docker", run: "docker compose up", icon: "container", tint: "#e0a83a" },
    ],
    links: [
      { title: "Локальный сервер", url: "localhost:8080", icon: "globe" },
      { title: "GitHub репозиторий", url: "github.com/artyom/aurora-api", icon: "github" },
      { title: "Дашборд деплоя", url: "fly.io/apps/aurora", icon: "rocket" },
      { title: "Swagger / OpenAPI", url: "localhost:8080/docs", icon: "file-json" },
    ],
    tasks: {
      todo: [
        { t: "Покрыть вебхуки идемпотентными ключами", pri: "high", due: "5 июн" },
        { t: "Описать схему ретраев в ADR", pri: "med", due: "8 июн" },
        { t: "Алёрты в Grafana на 5xx", pri: "low", due: "12 июн" },
      ],
      doing: [
        { t: "Воркер ретраев на tokio", pri: "high", due: "сегодня" },
        { t: "Рефактор слоя SQLx", pri: "med", due: "6 июн" },
      ],
      done: [
        { t: "Подключить трейсинг (tracing)", pri: "med", due: "2 июн" },
        { t: "CI на GitHub Actions", pri: "low", due: "30 мая" },
      ],
    },
    checklists: [
      { title: "Релиз v0.9", items: [
        { t: "Прогнать нагрузочные тесты", done: true },
        { t: "Обновить CHANGELOG", done: true },
        { t: "Проверить миграции на стейдже", done: true },
        { t: "Согласовать окно деплоя", done: false },
        { t: "Тег v0.9.0 + GitHub Release", done: false },
      ]},
      { title: "Перед каждым PR", items: [
        { t: "cargo fmt + clippy", done: true },
        { t: "Тесты зелёные", done: true },
        { t: "Описание PR заполнено", done: false },
      ]},
    ],
    creds: [
      { title: "Stripe (test)", type: "API-ключ", fields: [
        { k: "Ключ", v: "sk_test_51Hx9pQ2eZvKYlo2C", secret: true },
        { k: "Webhook", v: "whsec_8sd7f6g5h4j3k2l1", secret: true },
      ]},
      { title: "Postgres (local)", type: "Логин", fields: [
        { k: "Хост", v: "localhost:5432", secret: false },
        { k: "БД", v: "aurora_dev", secret: false },
        { k: "Юзер", v: "aurora", secret: false },
        { k: "Пароль", v: "p0stgr3s_l0cal", secret: true },
      ]},
      { title: "Fly.io deploy", type: "Токен", fields: [
        { k: "Токен", v: "fo1_aBcDeFgHiJkLmN0p", secret: true },
      ]},
      { title: "Сервер (SSH)", type: "SSH", fields: [
        { k: "Хост", v: "deploy@aurora.fly.dev", secret: false },
        { k: "Ключ", v: "~/.ssh/aurora_ed25519", secret: false },
      ]},
    ],
    note: "# Aurora API — рабочие заметки\n\nПлатёжный шлюз, **критичный** сервис. Главный фокус сейчас — *идемпотентность* вебхуков.\n\n## Архитектура ретраев\n\n- Входящий вебхук → запись в `webhook_events`\n- Воркер `retry_worker` тащит `pending` и шлёт дальше\n- Экспоненциальный backoff: `2^n` секунд, максимум 6 попыток\n\n```rust\nlet delay = Duration::from_secs(2u64.pow(attempt));\n```\n\n## Не забыть\n\n- [x] Трейсинг через `tracing`\n- [ ] Алёрты на рост `dead_letter`\n- [ ] ADR по схеме ретраев\n\n> Деплой только в окно 02:00–04:00 МСК, иначе ловим пик транзакций.\n\nДок по API: [localhost:8080/docs](http://localhost:8080/docs)",
  },

  {
    id: "nebula",
    name: "Nebula UI",
    emoji: "🎨",
    color: "#c77dff",
    pinned: true,
    status: "Активен",
    tags: ["react", "typescript", "vite"],
    path: "~/dev/nebula-ui",
    desc: "Дизайн-система и библиотека React-компонентов для внутренних продуктов. Токены, тёмная/светлая темы, Storybook и автогенерация документации.",
    git: { branch: "main", ahead: 0, behind: 0, dirty: 0, lastHash: "9c3b71d", lastMsg: "docs: примеры для DatePicker", staged: 0, untracked: 0 },
    commands: [
      { label: "Dev-сервер", run: "npm run dev", icon: "play", tint: "#c77dff" },
      { label: "Storybook", run: "npm run storybook", icon: "book-open", tint: "#f0616d" },
      { label: "Сборка", run: "npm run build", icon: "package", tint: "#5b9cff" },
      { label: "Линт", run: "npm run lint", icon: "sparkles", tint: "#3fb863" },
    ],
    links: [
      { title: "Dev-сервер", url: "localhost:3000", icon: "globe" },
      { title: "Storybook", url: "localhost:6006", icon: "book-open" },
      { title: "GitHub репозиторий", url: "github.com/artyom/nebula-ui", icon: "github" },
      { title: "npm пакет", url: "npmjs.com/package/nebula-ui", icon: "package" },
    ],
    tasks: {
      todo: [
        { t: "Компонент Combobox с виртуализацией", pri: "high", due: "9 июн" },
        { t: "Токены spacing → CSS-переменные", pri: "med", due: "11 июн" },
      ],
      doing: [
        { t: "Перенести иконки на единый sprite", pri: "med", due: "6 июн" },
      ],
      done: [
        { t: "DatePicker: примеры в Storybook", pri: "low", due: "3 июн" },
        { t: "Контрастность по WCAG AA", pri: "high", due: "1 июн" },
        { t: "Тёмная тема для всех компонентов", pri: "med", due: "28 мая" },
      ],
    },
    checklists: [
      { title: "Релиз 2.0", items: [
        { t: "Все компоненты в Storybook", done: true },
        { t: "Миграционный гайд 1.x → 2.0", done: false },
        { t: "Визуальные регрессы (Chromatic)", done: true },
        { t: "Обновить README", done: false },
      ]},
      { title: "Доступность", items: [
        { t: "Фокус-кольца везде", done: true },
        { t: "aria-атрибуты для модалок", done: true },
        { t: "Навигация с клавиатуры", done: true },
        { t: "Проверка скринридером", done: false },
      ]},
    ],
    creds: [
      { title: "npm publish", type: "Токен", fields: [
        { k: "Токен", v: "npm_7Hk2Lp9QwErTy8Zx", secret: true },
      ]},
      { title: "Chromatic", type: "API-ключ", fields: [
        { k: "Проект", v: "chpt_neb1ula", secret: false },
        { k: "Токен", v: "chpt_a9b8c7d6e5f4", secret: true },
      ]},
      { title: "Figma", type: "Логин", fields: [
        { k: "Файл", v: "figma.com/file/nebula-ds", secret: false },
        { k: "Токен", v: "figd_xY12abCD34efGH56", secret: true },
      ]},
    ],
    note: "# Nebula UI\n\nДизайн-система. Принцип: **токены первичны**, компоненты вторичны.\n\n## Правила\n\n1. Никаких хардкод-цветов — только `var(--…)`\n2. Каждый компонент — со Storybook-историей\n3. Тёмная/светлая темы из коробки\n\n## Релиз 2.0 — что ломаем\n\n- `Button` теперь без пропа `kind`, только `variant`\n- Иконки переехали в `@nebula/icons`\n\n```tsx\n<Button variant=\"primary\">Сохранить</Button>\n```\n\n> Перед публикацией обязательно прогнать Chromatic — визуальные регрессы ловятся только там.",
  },

  {
    id: "pulse",
    name: "Pulse Analytics",
    emoji: "📊",
    color: "#3fb863",
    pinned: true,
    status: "Активен",
    tags: ["next", "clickhouse", "grafana"],
    path: "~/dev/pulse",
    desc: "Аналитический дашборд продуктовых метрик. Next.js на фронте, ClickHouse под событиями, ETL на расписании. Считаем retention, воронки и LTV в реальном времени.",
    git: { branch: "main", ahead: 1, behind: 0, dirty: 0, lastHash: "4e8a0b2", lastMsg: "feat: когортный retention за 90 дней", staged: 0, untracked: 0 },
    commands: [
      { label: "Dev-сервер", run: "npm run dev", icon: "play", tint: "#3fb863" },
      { label: "ETL прогон", run: "python etl/run.py", icon: "refresh-cw", tint: "#5b9cff" },
      { label: "Сборка", run: "npm run build", icon: "package", tint: "#c77dff" },
    ],
    links: [
      { title: "Дашборд", url: "localhost:3000", icon: "globe" },
      { title: "ClickHouse UI", url: "localhost:8123/play", icon: "database" },
      { title: "Grafana", url: "grafana.pulse.local", icon: "activity" },
      { title: "GitHub репозиторий", url: "github.com/artyom/pulse", icon: "github" },
    ],
    tasks: {
      todo: [
        { t: "Воронка онбординга по шагам", pri: "high", due: "10 июн" },
        { t: "Кэш тяжёлых запросов в Redis", pri: "med", due: "13 июн" },
      ],
      doing: [
        { t: "Экспорт отчёта в CSV/PDF", pri: "med", due: "7 июн" },
      ],
      done: [
        { t: "Когортный retention 90д", pri: "high", due: "2 июн" },
        { t: "Партиционирование событий по дням", pri: "med", due: "29 мая" },
      ],
    },
    checklists: [
      { title: "Качество данных", items: [
        { t: "Дедуп событий в ETL", done: true },
        { t: "Проверка схемы при импорте", done: true },
        { t: "Алёрт на пропуск ночного ETL", done: false },
      ]},
    ],
    creds: [
      { title: "ClickHouse", type: "Логин", fields: [
        { k: "Хост", v: "localhost:8123", secret: false },
        { k: "Юзер", v: "analytics", secret: false },
        { k: "Пароль", v: "ch_an4lyt1cs", secret: true },
      ]},
      { title: "Grafana API", type: "Токен", fields: [
        { k: "Токен", v: "glsa_Kp9Qw2ErTy8Zx", secret: true },
      ]},
    ],
    note: "# Pulse Analytics\n\nПродуктовая аналитика на ClickHouse.\n\n## Метрики\n\n- **Retention** — когортный, по дням 1/7/30/90\n- **Воронки** — событийные, шаг за шагом\n- **LTV** — накопленный по платящим\n\n## ETL\n\nНочной прогон в 03:00, дедуп по `event_id`.\n\n```sql\nSELECT count() FROM events WHERE date = today();\n```\n\n> Если ночной ETL не прошёл — дашборды покажут вчерашние данные. Нужен алёрт.",
  },

  {
    id: "helix",
    name: "Helix Bot",
    emoji: "🤖",
    color: "#e0a83a",
    pinned: false,
    status: "Пауза",
    tags: ["python", "aiogram", "redis"],
    path: "~/dev/helix-bot",
    desc: "Телеграм-бот для трекинга привычек с напоминаниями и стриками. aiogram 3, хранилище в Redis. Сейчас на паузе — жду фидбэк от первых пользователей.",
    git: { branch: "main", ahead: 0, behind: 0, dirty: 0, lastHash: "7b21c9e", lastMsg: "fix: таймзоны напоминаний", staged: 0, untracked: 0 },
    commands: [
      { label: "Запустить бота", run: "python -m helix", icon: "play", tint: "#e0a83a" },
      { label: "Redis CLI", run: "redis-cli", icon: "database", tint: "#f0616d" },
    ],
    links: [
      { title: "Бот в Telegram", url: "t.me/helix_habits_bot", icon: "send" },
      { title: "GitHub репозиторий", url: "github.com/artyom/helix-bot", icon: "github" },
    ],
    tasks: {
      todo: [
        { t: "Недельная статистика привычек", pri: "med", due: "—" },
        { t: "Экспорт данных пользователя", pri: "low", due: "—" },
      ],
      doing: [],
      done: [
        { t: "Напоминания с учётом таймзоны", pri: "high", due: "25 мая" },
        { t: "Стрики и заморозка стрика", pri: "med", due: "20 мая" },
      ],
    },
    checklists: [
      { title: "MVP", items: [
        { t: "Создание привычек", done: true },
        { t: "Напоминания", done: true },
        { t: "Стрики", done: true },
        { t: "Онбординг-сценарий", done: false },
      ]},
    ],
    creds: [
      { title: "Telegram Bot", type: "Токен", fields: [
        { k: "Токен", v: "7891234567:AAH-bOtT0k3nXyZ", secret: true },
      ]},
      { title: "Redis", type: "Логин", fields: [
        { k: "Хост", v: "localhost:6379", secret: false },
        { k: "Пароль", v: "r3d1s_h3l1x", secret: true },
      ]},
    ],
    note: "# Helix Bot\n\nТрекер привычек в Telegram. На паузе ⏸\n\n## Идеи\n\n- Социальные стрики (соревнование с друзьями)\n- Гибкие напоминания (умное время)\n\n> Дождаться фидбэка от первых 20 пользователей, потом решать про развитие.",
  },

  {
    id: "market",
    name: "Market Core",
    emoji: "🛒",
    color: "#f0616d",
    pinned: false,
    status: "Активен",
    tags: ["go", "grpc", "kafka"],
    path: "~/dev/market-core",
    desc: "Ядро маркетплейса: каталог, заказы, инвентарь. Микросервисы на Go, gRPC между сервисами, Kafka для событий. Высоконагруженная часть платформы.",
    git: { branch: "feature/inventory-lock", ahead: 4, behind: 1, dirty: 7, lastHash: "c5d8f31", lastMsg: "wip: оптимистичные блокировки склада", staged: 2, untracked: 5 },
    commands: [
      { label: "Запустить", run: "make run", icon: "play", tint: "#f0616d" },
      { label: "Тесты", run: "go test ./...", icon: "flask-conical", tint: "#3fb863" },
      { label: "Kafka up", run: "docker compose up kafka", icon: "container", tint: "#e0a83a" },
      { label: "Proto gen", run: "make proto", icon: "file-code", tint: "#5b9cff" },
    ],
    links: [
      { title: "API Gateway", url: "localhost:8000", icon: "globe" },
      { title: "Kafka UI", url: "localhost:9000", icon: "activity" },
      { title: "GitHub репозиторий", url: "github.com/artyom/market-core", icon: "github" },
      { title: "Дашборд деплоя", url: "argocd.market.io", icon: "rocket" },
    ],
    tasks: {
      todo: [
        { t: "Оптимистичные блокировки на складе", pri: "high", due: "6 июн" },
        { t: "Идемпотентность создания заказа", pri: "high", due: "9 июн" },
        { t: "Метрики латентности gRPC", pri: "med", due: "12 июн" },
      ],
      doing: [
        { t: "Saga для отмены заказа", pri: "high", due: "сегодня" },
        { t: "Контракт-тесты на proto", pri: "med", due: "7 июн" },
      ],
      done: [
        { t: "Outbox-паттерн для Kafka", pri: "high", due: "1 июн" },
      ],
    },
    checklists: [
      { title: "Готовность к нагрузке", items: [
        { t: "Нагрузочный профиль (k6)", done: true },
        { t: "Graceful shutdown сервисов", done: true },
        { t: "Circuit breaker между сервисами", done: false },
        { t: "Rate limiting на gateway", done: false },
        { t: "Бюджет ошибок (SLO)", done: false },
      ]},
    ],
    creds: [
      { title: "Kafka (SASL)", type: "Логин", fields: [
        { k: "Брокер", v: "localhost:9092", secret: false },
        { k: "Юзер", v: "market", secret: false },
        { k: "Пароль", v: "k4fk4_s3cr3t", secret: true },
      ]},
      { title: "Деплой (SSH)", type: "SSH", fields: [
        { k: "Хост", v: "deploy@market.io", secret: false },
        { k: "Ключ", v: "~/.ssh/market_rsa", secret: false },
      ]},
      { title: "ArgoCD", type: "Токен", fields: [
        { k: "Токен", v: "argo_8Hj2Kp9Lm3Nq7Rs", secret: true },
      ]},
    ],
    note: "# Market Core\n\nЯдро маркетплейса. **Высокая нагрузка**, цена ошибки высокая.\n\n## Сейчас в работе\n\nОптимистичные блокировки склада — чтобы два заказа не списали один товар.\n\n```go\nUPDATE inventory SET qty = qty - 1, version = version + 1\nWHERE id = $1 AND version = $2\n```\n\n## Принципы\n\n- Outbox для всех событий в Kafka\n- Saga для распределённых транзакций\n- Контракт-тесты на каждый proto\n\n> 7 изменённых файлов — не забыть разбить на атомарные коммиты перед push.",
  },

  {
    id: "drift",
    name: "Drift Mobile",
    emoji: "📱",
    color: "#5b9cff",
    pinned: false,
    status: "Активен",
    tags: ["react-native", "expo", "ts"],
    path: "~/dev/drift-mobile",
    desc: "Мобильное приложение для совместных заметок-путешествий. React Native + Expo, синхронизация офлайн-первая. Готовим бету в TestFlight.",
    git: { branch: "release/1.2", ahead: 0, behind: 0, dirty: 2, lastHash: "2a9e4f7", lastMsg: "ui: новый онбординг-экран", staged: 0, untracked: 2 },
    commands: [
      { label: "Expo Start", run: "npx expo start", icon: "play", tint: "#5b9cff" },
      { label: "iOS симулятор", run: "npx expo run:ios", icon: "smartphone", tint: "#c77dff" },
      { label: "EAS Build", run: "eas build -p ios", icon: "package", tint: "#3fb863" },
    ],
    links: [
      { title: "Expo Dev", url: "localhost:8081", icon: "globe" },
      { title: "TestFlight", url: "appstoreconnect.apple.com", icon: "rocket" },
      { title: "GitHub репозиторий", url: "github.com/artyom/drift-mobile", icon: "github" },
    ],
    tasks: {
      todo: [
        { t: "Офлайн-конфликты: last-write-wins", pri: "high", due: "11 июн" },
        { t: "Пуш-уведомления (Expo)", pri: "med", due: "14 июн" },
      ],
      doing: [
        { t: "Онбординг: 3 экрана", pri: "med", due: "8 июн" },
      ],
      done: [
        { t: "Тёмная тема", pri: "low", due: "30 мая" },
        { t: "Карта с маркерами мест", pri: "high", due: "27 мая" },
      ],
    },
    checklists: [
      { title: "Бета в TestFlight", items: [
        { t: "Иконка и сплэш", done: true },
        { t: "Скриншоты для стора", done: false },
        { t: "Политика конфиденциальности", done: false },
        { t: "EAS-профиль production", done: true },
      ]},
    ],
    creds: [
      { title: "Expo", type: "Токен", fields: [
        { k: "Токен", v: "exp_4Kp9Lm3Nq7RsTy8Z", secret: true },
      ]},
      { title: "Apple Developer", type: "Логин", fields: [
        { k: "Team ID", v: "A1B2C3D4E5", secret: false },
        { k: "App ID", v: "com.artyom.drift", secret: false },
      ]},
    ],
    note: "# Drift Mobile\n\nЗаметки-путешествия, **офлайн-первый** подход.\n\n## Синхронизация\n\nCRDT слишком тяжело для MVP → пока last-write-wins по `updatedAt`.\n\n## Перед бетой\n\n- [x] EAS production-профиль\n- [ ] Скриншоты 6.7\" и 5.5\"\n- [ ] Privacy policy\n\n> TestFlight ревью занимает ~1 день, закладываем время.",
  },
];
