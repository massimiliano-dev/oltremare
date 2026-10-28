// Lettura dell'output di `git diff`: file, blocchi, righe numerate ed evidenziazione delle parole cambiate.

export type Seg = { text: string; hot: boolean };
export type DLine = { kind: "ctx" | "add" | "del"; old?: number; new?: number; text: string; segs?: Seg[] };
export type Hunk = { header: string; scope: string; lines: DLine[] };
export type DFile = {
  path: string;
  from?: string;
  status: "modified" | "added" | "deleted" | "renamed";
  binary: boolean;
  add: number;
  del: number;
  hunks: Hunk[];
};
export type SplitRow = { kind: "hunk"; hunk: Hunk } | { kind: "pair"; left?: DLine; right?: DLine };

const HUNK = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@ ?(.*)$/;

export function parseDiff(text: string): DFile[] {
  const files: DFile[] = [];
  let f: DFile | null = null;
  let h: Hunk | null = null;
  let o = 0;
  let n = 0;
  for (const raw of text.split("\n")) {
    if (raw.startsWith("diff --git ")) {
      const m = raw.match(/^diff --git a\/(.+) b\/(.+)$/);
      f = { path: m?.[2] ?? raw.slice(11), status: "modified", binary: false, add: 0, del: 0, hunks: [] };
      h = null;
      files.push(f);
      continue;
    }
    if (!f) continue;
    if (!h) {
      if (raw.startsWith("new file")) f.status = "added";
      else if (raw.startsWith("deleted file")) f.status = "deleted";
      else if (raw.startsWith("rename from ")) {
        f.status = "renamed";
        f.from = raw.slice(12);
      } else if (raw.startsWith("rename to ")) f.path = raw.slice(10);
      else if (raw.startsWith("Binary files")) f.binary = true;
      else if (raw.startsWith("+++ ") && raw !== "+++ /dev/null") f.path = raw.slice(6);
    }
    const m = raw.match(HUNK);
    if (m) {
      o = +m[1];
      n = +m[2];
      h = { header: raw, scope: m[3], lines: [] };
      f.hunks.push(h);
      continue;
    }
    if (!h || raw.startsWith("\\")) continue;
    const c = raw[0];
    if (c === "+") {
      h.lines.push({ kind: "add", new: n++, text: raw.slice(1) });
      f.add++;
    } else if (c === "-") {
      h.lines.push({ kind: "del", old: o++, text: raw.slice(1) });
      f.del++;
    } else if (c === " ") {
      h.lines.push({ kind: "ctx", old: o++, new: n++, text: raw.slice(1) });
    }
  }
  for (const file of files) for (const hunk of file.hunks) markWords(hunk.lines);
  return files;
}

/** Nei gruppi «tolte poi aggiunte» accoppia le righe e mette in risalto solo la parte cambiata. */
function markWords(lines: DLine[]) {
  let i = 0;
  while (i < lines.length) {
    if (lines[i].kind !== "del") {
      i++;
      continue;
    }
    const d0 = i;
    while (i < lines.length && lines[i].kind === "del") i++;
    const a0 = i;
    while (i < lines.length && lines[i].kind === "add") i++;
    const pairs = Math.min(a0 - d0, i - a0);
    for (let k = 0; k < pairs; k++) {
      const [ds, as] = wordDiff(lines[d0 + k].text, lines[a0 + k].text);
      if (ds && as) {
        lines[d0 + k].segs = ds;
        lines[a0 + k].segs = as;
      }
    }
  }
}

const TOKENS = /\w+|\s+|[^\w\s]/g;

function wordDiff(a: string, b: string): [Seg[] | null, Seg[] | null] {
  if (a.length > 400 || b.length > 400) return [null, null];
  const ta = a.match(TOKENS) ?? [];
  const tb = b.match(TOKENS) ?? [];
  let p = 0;
  while (p < ta.length && p < tb.length && ta[p] === tb[p]) p++;
  let s = 0;
  while (s < ta.length - p && s < tb.length - p && ta[ta.length - 1 - s] === tb[tb.length - 1 - s]) s++;
  const midA = ta.slice(p, ta.length - s).join("");
  const midB = tb.slice(p, tb.length - s).join("");
  // Se cambia quasi tutta la riga l'evidenziazione non aiuta
  if (midA.length + midB.length > 0.8 * (a.length + b.length)) return [null, null];
  const pre = ta.slice(0, p).join("");
  const suf = ta.slice(ta.length - s).join("");
  const seg = (mid: string): Seg[] => [
    { text: pre, hot: false },
    { text: mid, hot: true },
    { text: suf, hot: false },
  ].filter((x) => x.text);
  return [seg(midA), seg(midB)];
}

/** Righe affiancate: a sinistra com'era, a destra com'è. */
export function splitRows(file: DFile): SplitRow[] {
  const rows: SplitRow[] = [];
  for (const hunk of file.hunks) {
    rows.push({ kind: "hunk", hunk });
    const ls = hunk.lines;
    let i = 0;
    while (i < ls.length) {
      if (ls[i].kind === "ctx") {
        rows.push({ kind: "pair", left: ls[i], right: ls[i] });
        i++;
        continue;
      }
      const dels: DLine[] = [];
      const adds: DLine[] = [];
      while (i < ls.length && ls[i].kind === "del") dels.push(ls[i++]);
      while (i < ls.length && ls[i].kind === "add") adds.push(ls[i++]);
      for (let k = 0; k < Math.max(dels.length, adds.length); k++) rows.push({ kind: "pair", left: dels[k], right: adds[k] });
    }
  }
  return rows;
}
