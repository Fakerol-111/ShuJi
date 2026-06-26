/** @type {import('lint-staged').Configuration} */
export default {
  'shuji-app/src/**/*.{ts,tsx}': (files) =>
    `npm exec --prefix shuji-app -- prettier --write ${files.map((f) => `"${f}"`).join(' ')}`,
  'shuji-app/src-tauri/**/*.rs': (files) =>
    `rustfmt --edition 2021 ${files.map((f) => `"${f}"`).join(' ')}`,
};
