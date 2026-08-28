#!/usr/bin/env bash
# Update tablec-cli/webui/vendor/lit.js to the latest stable Lit release.
#
# Usage:
#   ./update-lit.sh                       # fetch latest from GitHub
#   ./update-lit.sh 3.3.3                 # fetch a specific version
#
# The vendored bundle must be SELF-CONTAINED (no `import "..."` statements)
# because the webui has zero runtime network dependencies. esm.sh's standard
# bundle of Lit has at least one external chunk import (a css-tag chunk
# pulled out of @lit/reactive-element). esm.sh itself sometimes serves a
# *redirector* file that just re-exports from another chunk — we follow
# those until we reach the actual implementation.
#
# Final assembly:
#   1. Inline each chunk's body so its `var X = ...` declarations land at
#      the combined module's top-level scope.
#   2. Strip every `export{...}` (they were for the chunk's own module —
#      useless now that everything shares one scope).
#   3. Strip every `export * from "..."` (redirector syntax — useless after
#      we inlined the target).
#   4. Replace the bundle's `import{X as Y,...}from"...";` statements with
#      nothing — but record the `Y=X` aliases and emit a single shim line
#      `var Y=X,...;` so the bundle's references to the aliased names still
#      resolve.

set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# ---------------------------------------------------------------------------
# 1. Resolve version
# ---------------------------------------------------------------------------

if [[ $# -ge 1 ]]; then
  VERSION="$1"
else
  # Pull the most recent releases and pick the one whose tag is exactly
  # `lit@X.Y.Z`. The monorepo also releases `@lit-labs/*` and `lit-html`
  # separately, so we can't just use /releases/latest.
  VERSION=$(curl -sSL 'https://api.github.com/repos/lit/lit/releases?per_page=20' \
            | grep -oE '"tag_name":\s*"lit@[^"]+"' \
            | head -1 \
            | sed -E 's/.*"lit@([^"]+)".*/\1/')
  if [[ -z "$VERSION" ]]; then
    echo "ERROR: could not resolve latest lit version from GitHub API" >&2
    exit 1
  fi
fi
echo "→ fetching lit@${VERSION}"

# ---------------------------------------------------------------------------
# 2. Fetch bundle + chunks (following redirectors)
# ---------------------------------------------------------------------------

BUNDLE_URL="https://esm.sh/lit@${VERSION}/es2022/lit.bundle.mjs?target=es2022"
curl -sSL -o "$TMP/bundle.mjs" "$BUNDLE_URL"

# Extract chunk paths the bundle imports from.
# Pattern: from"/<path>?target=es2022"
mapfile -t CHUNK_PATHS < <(
  grep -oE 'from"[^"]*target=es2022"' "$TMP/bundle.mjs" \
    | sed -E 's|from"(/[^"]+)\?target=es2022"|\1|' \
    | sort -u
)

if [[ ${#CHUNK_PATHS[@]} -eq 0 ]]; then
  echo "ERROR: bundle has no external chunk imports — nothing to inline." >&2
  echo "       This likely means esm.sh changed their bundle layout." >&2
  echo "       Open update-lit.sh and inspect the new chunking scheme." >&2
  exit 1
fi

echo "  ${#CHUNK_PATHS[@]} external chunk(s):"

# For each chunk path, walk the redirector chain until we reach the
# implementation file (one that doesn't just `export * from` somewhere).
resolve_chunk() {
  local path="$1"
  local url="https://esm.sh${path}?target=es2022"
  local body_file="$TMP/resolved-$(echo "$path" | tr '/@.' '_').mjs"
  curl -sSL -o "$body_file" "$url"
  # If the file just re-exports from elsewhere, follow it. The capture
  # uses `from "..."` (not `"/...`) so we keep the leading slash on the
  # path — otherwise URLs collapse into `https://esm.shlit@...` and curl
  # mis-reads them as host `lit`.
  local next
  next=$(grep -oE 'export \* from "[^"]+"' "$body_file" \
         | head -1 | sed -E 's|.*from "([^"]+)".*|\1|' || true)
  while [[ -n "$next" ]]; do
    curl -sSL -o "$body_file" "https://esm.sh${next}?target=es2022"
    next=$(grep -oE 'export \* from "[^"]+"' "$body_file" \
           | head -1 | sed -E 's|.*from "([^"]+)".*|\1|' || true)
  done
  echo "$body_file"
}

CHUNK_BODIES=()
for path in "${CHUNK_PATHS[@]}"; do
  echo "    - $path"
  CHUNK_BODIES+=("$(resolve_chunk "$path")")
done

# ---------------------------------------------------------------------------
# 3. Assemble self-contained bundle
# ---------------------------------------------------------------------------

ASSEMBLED="$TMP/assembled.mjs"
: > "$ASSEMBLED"

# 3a. Inline every chunk's body. Strip its header comment line, its trailing
#     sourceMappingURL line, and any `export{...}` or `export * from "..."`
#     statements — none of those have any meaning once we're all one module.
for chunk in "${CHUNK_BODIES[@]}"; do
  sed -E -e '1d' \
         -e '/^\/\/# sourceMappingURL/d' \
         -e 's/;export\{[^}]*\};/;/g' \
         -e '/^export \* from "[^"]+";$/d' \
         "$chunk" >> "$ASSEMBLED"
  echo >> "$ASSEMBLED"
done

# 3b. Capture the alias map: `import{X as Y,...}from"...";` → `Y=X,...`
#     and emit a single shim line at the end of the chunk prefix.
ALIASES=$(grep -oE 'import\{[^}]*\}' "$TMP/bundle.mjs" \
          | grep -oE '[A-Za-z_$][A-Za-z0-9_$]* as [A-Za-z_$][A-Za-z0-9_$]*' \
          | sed -E 's/^([A-Za-z_$][A-Za-z0-9_$]*) as ([A-Za-z_$][A-Za-z0-9_$]*)/\2=\1/' \
          | tr '\n' ',' \
          | sed 's/,$//')

if [[ -n "$ALIASES" ]]; then
  echo "var ${ALIASES};" >> "$ASSEMBLED"
  echo "  aliases: ${ALIASES}"
fi

# 3c. Strip the import statements from the bundle body but keep everything
#     else, including any continuation lines (template literals / regexes
#     may span multiple lines — deleting the import line would break them).
sed -E -e '1d' \
       -e 's|import\{[^}]*\}from"[^"]*";||g' \
       -e '/^\/\/# sourceMappingURL/d' \
       "$TMP/bundle.mjs" >> "$ASSEMBLED"

# ---------------------------------------------------------------------------
# 4. Write vendor/lit.js + sanity check
# ---------------------------------------------------------------------------

cp "$ASSEMBLED" "$HERE/lit.js"
SIZE=$(wc -c < "$HERE/lit.js")
echo "→ wrote $HERE/lit.js ($SIZE bytes)"

# Parse-check: bundle must be syntactically valid ESM.
cp "$HERE/lit.js" "$TMP/check.mjs"
if ! node --input-type=module --check < "$TMP/check.mjs" 2>"$TMP/parse.err"; then
  echo "ERROR: vendored bundle does not parse as ESM." >&2
  cat "$TMP/parse.err" >&2
  exit 1
fi

# String-check: every export our app.js depends on must be present in the
# final export list.
for sym in LitElement html css CSSResult; do
  if ! grep -qE "(as ${sym}\b|,${sym}\b|^${sym}\b)" "$HERE/lit.js"; then
    echo "ERROR: expected export '${sym}' missing from bundle" >&2
    exit 1
  fi
done

# String-check: no `import "..."` statements survive (the bundle must be
# fully self-contained for offline use).
if grep -E '^import\b' "$HERE/lit.js"; then
  echo "ERROR: bundle still has live import statements" >&2
  exit 1
fi

echo "✓ lit@${VERSION} vendored (${SIZE} bytes)"
echo "  next: rebuild and run 'cargo test -p tablec-cli'"