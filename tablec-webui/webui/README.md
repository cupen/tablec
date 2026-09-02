# tablec webui — frontend

Zero-dependency-at-runtime SPA: built with pnpm + Vite + TypeScript + Lit +
Web Awesome, then **embedded into the Rust binary** via `include_dir!`
(`src/handlers.rs`). `cargo build` works without node — the compiled `dist/`
is committed.

## Commands (run in this directory)

```bash
pnpm install        # first setup
pnpm check          # tsc --noEmit typecheck
pnpm dev            # Vite dev server on :5173, proxies /api → 127.0.0.1:8080
pnpm build          # emit dist/ (then `cargo build` to re-embed)
```

## Dev workflow

1. Start the backend: `target/debug/tablec webui --no-browser --port 8080 -d <dir>` (repo root).
2. `pnpm dev` and open http://localhost:5173 — HMR for frontend edits, API
   calls proxied to the Rust server. No Rust rebuild needed.
3. When done, `pnpm build` + `cargo build` so the shipped binary serves the
   new frontend.

## Layout

- `src/main.ts` — entry: Web Awesome theme + component imports, global styles.
- `src/styles.css` — design tokens (Inkwell/Blueprint themes), reset, and the
  bridge that maps our tokens onto Web Awesome's `--wa-*` variables.
- `src/store.ts` — central store + change bus + `StoreSub` controller.
- `src/api.ts` — typed fetch helpers; `src/theme.ts` — theme controller.
- `src/components/` — one Lit element per UI region (app-shell, file-list,
  file-preview, build-panel, status-bar).
- `dist/` — Vite build output; **committed** because the Rust build embeds it.
