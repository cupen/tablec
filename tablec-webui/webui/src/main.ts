// tablec webui — entry point.
//
// Visual system: dark "Inkwell" / light "Blueprint" via [data-theme] on
// <html>, bridged into Web Awesome's --wa-* tokens in styles.css.

// Web Awesome: default theme tokens (component styles ship with each
// component import) — loaded before any wa-* element upgrades.
import '@awesome.me/webawesome/dist/styles/themes/default.css';

// Web Awesome components (cherry-picked; each import registers its element).
import '@awesome.me/webawesome/dist/components/button/button.js';
import '@awesome.me/webawesome/dist/components/button-group/button-group.js';
import '@awesome.me/webawesome/dist/components/badge/badge.js';
import '@awesome.me/webawesome/dist/components/select/select.js';
import '@awesome.me/webawesome/dist/components/option/option.js';
import '@awesome.me/webawesome/dist/components/checkbox/checkbox.js';

// App-wide styles (design tokens, reset, app-shell layout).
import './styles.css';

// App components.
import './components/app-shell.js';
