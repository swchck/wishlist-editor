# Wishlist Editor

Десктопное приложение (macOS, Windows) для редактирования вишлиста, собранного из
[wishlist-template](https://github.com/swchck/wishlist-template): карточки подарков
правятся формой и визуальным markdown-редактором, без терминала и без знания git.

Стек: [Tauri 2](https://tauri.app), Vue 3, [shadcn-vue](https://www.shadcn-vue.com),
[Milkdown](https://milkdown.dev).

## Разработка

```bash
npm install
npm run tauri dev   # приложение целиком
npm run dev         # только интерфейс в браузере, IPC подменяется тестовыми карточками
npm run check       # типы, линтеры, тесты фронтенда и Rust
```

Проверить, что редактор не меняет файлы при открытии и сохранении, на своём вишлисте:

```bash
WISHLIST_ITEMS=/path/to/wishlist/src/content/items npx vitest run src/lib/wishlist/item-file.test.ts
```

## Как устроено

- `src-tauri/src/workspace.rs` — доступ к файлам вишлиста. Вебвью передаёт только имя
  карточки (slug), путь строится и проверяется в Rust.
- `src/lib/wishlist/` — схема карточки (зеркало `src/content.config.ts` шаблона) и
  разбор фронтматтера, который сохраняет порядок полей, кавычки и комментарии.
- `src/stores/wishlist.ts` — открытый вишлист и автосохранение.

Формат вишлиста версионируется полем `schemaVersion` в `wishlist.config.json`. Если оно
не совпадает с `SCHEMA_VERSION` в `src/lib/wishlist/schema.ts`, вишлист открывается только
для чтения.
