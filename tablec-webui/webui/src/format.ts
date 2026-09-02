// Pure formatting helpers shared across components.

// 0 → A, 25 → Z, 26 → AA, 51 → AZ, 52 → BA (0-indexed spreadsheet cols).
export function colLetter(idx: number): string {
  let s = '';
  let n = idx;
  while (true) {
    s = String.fromCharCode(65 + (n % 26)) + s;
    n = Math.floor(n / 26) - 1;
    if (n < 0) break;
  }
  return s;
}

export function extOf(name: string): string {
  const i = name.lastIndexOf('.');
  return i >= 0 ? name.slice(i + 1).toLowerCase() : '';
}

export function baseName(p: string): string {
  if (!p) return '';
  const i = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
  return i >= 0 ? p.slice(i + 1) : p;
}

export function truncErr(e: string): string {
  return e.length > 60 ? e.slice(0, 57) + '…' : e;
}

export function humanSize(n: number): string {
  const u = ['B', 'KB', 'MB', 'GB'];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < u.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(i ? 1 : 0)} ${u[i]}`;
}

/** `FieldType` is a serde-tagged enum: "Int32" or {VariantName: {…}}. */
export function typeNameOf(t: unknown): string {
  if (t == null) return '?';
  if (typeof t === 'string') return t.toLowerCase();
  if (typeof t === 'object') {
    const k = Object.keys(t as object)[0];
    return k ? k.toLowerCase() : '?';
  }
  return '?';
}

// CSS class for a raw cell based on its runtime type (raw view).
export function cellClass(cell: unknown): string {
  if (cell == null) return 'null';
  if (typeof cell === 'number') return 'num';
  if (typeof cell === 'boolean') return 'bool';
  if (typeof cell === 'object' && 'Float' in (cell as object)) return 'num';
  if (typeof cell === 'object' && 'Bool' in (cell as object)) return 'bool';
  return '';
}

// CSS class for a typed cell value (parsed view).
export function cellClassTyped(v: unknown): string {
  if (v == null) return '';
  if (typeof v === 'number') return 'num';
  if (typeof v === 'boolean') return 'bool';
  return '';
}
