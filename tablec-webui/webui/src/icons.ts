// Inline SVG icons, themed via currentColor.
import { html } from 'lit';
import type { TemplateResult } from 'lit';

export const SUN_ICON: TemplateResult = html`
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
       stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <circle cx="12" cy="12" r="4"/>
    <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l1.41-1.41M17.66 6.34l1.41-1.41"/>
  </svg>`;

export const MOON_ICON: TemplateResult = html`
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
       stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/>
  </svg>`;

export const RELOAD_ICON: TemplateResult = html`
  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor"
       stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path d="M3 12a9 9 0 0 1 15.5-6.3L21 8"/>
    <path d="M21 3v5h-5"/>
    <path d="M21 12a9 9 0 0 1-15.5 6.3L3 16"/>
    <path d="M3 21v-5h5"/>
  </svg>`;
