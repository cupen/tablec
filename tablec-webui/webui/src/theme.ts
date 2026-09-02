// Theme controller — flips [data-theme] on <html> and persists to
// localStorage. Components don't re-render on theme change (CSS vars flip),
// but the toggle button does to swap the sun/moon icon.
//
// Web Awesome adapts its own palette to light/dark via the `wa-light` /
// `wa-dark` classes, so both attributes are kept in sync here and in the
// inline <head> script in index.html.

import type { ReactiveController, ReactiveControllerHost } from 'lit';

const THEME_KEY = 'tablec-theme';

type Theme = 'dark' | 'light';

function apply(theme: Theme) {
  const root = document.documentElement;
  root.setAttribute('data-theme', theme);
  root.classList.toggle('wa-dark', theme === 'dark');
  root.classList.toggle('wa-light', theme === 'light');
}

export class ThemeCtrl implements ReactiveController {
  private host: ReactiveControllerHost;
  theme: Theme = 'dark';

  constructor(host: ReactiveControllerHost) {
    host.addController(this);
    this.host = host;
  }
  hostConnected() {
    let stored: string | null = null;
    try {
      stored = localStorage.getItem(THEME_KEY);
    } catch {
      /* private mode */
    }
    this.theme = stored === 'light' ? 'light' : 'dark';
    apply(this.theme);
  }
  toggle() {
    this.theme = this.theme === 'dark' ? 'light' : 'dark';
    apply(this.theme);
    try {
      localStorage.setItem(THEME_KEY, this.theme);
    } catch {
      /* ignore */
    }
    this.host.requestUpdate();
  }
}
