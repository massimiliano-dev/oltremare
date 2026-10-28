// Markdown leggero per le risposte: paragrafi, elenchi, titoli, grassetto, corsivo, barrato,
// codice in linea e a blocchi, citazioni, tabelle, link http(s). Tutto il resto resta testo.
import { escapeHtml } from "./format";

function inline(s: string): string {
  // I frammenti di codice si proteggono prima delle altre sostituzioni
  const codes: string[] = [];
  let out = s.replace(/`([^`]+)`/g, (_, c) => {
    codes.push(c);
    return `\u0000${codes.length - 1}\u0000`;
  });
  out = out
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    .replace(/~~(.+?)~~/g, "<del>$1</del>")
    .replace(/(^|[^*\w])\*(?!\s)(.+?)\*(?!\w)/g, "$1<em>$2</em>")
    .replace(/\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g, '<a href="$2" target="_blank" rel="noreferrer">$1</a>')
    .replace(/(^|[\s(])(https?:\/\/[^\s<)]+)/g, '$1<a href="$2" target="_blank" rel="noreferrer">$2</a>');
  return out.replace(/\u0000(\d+)\u0000/g, (_, i) => `<code>${codes[Number(i)]}</code>`);
}

/** Celle di una riga di tabella: «| a | b |» → ["a", "b"] (le barre dentro il codice restano). */
function cells(line: string): string[] {
  let t = line.trim();
  if (t.startsWith("|")) t = t.slice(1);
  if (t.endsWith("|") && !t.endsWith("\\|")) t = t.slice(0, -1);
  const out: string[] = [];
  let cur = "";
  let inCode = false;
  for (let i = 0; i < t.length; i++) {
    const c = t[i];
    if (c === "`") inCode = !inCode;
    if (c === "\\" && t[i + 1] === "|") {
      cur += "|";
      i++;
      continue;
    }
    if (c === "|" && !inCode) {
      out.push(cur.trim());
      cur = "";
      continue;
    }
    cur += c;
  }
  out.push(cur.trim());
  return out;
}

const SEPARATOR = /^\s*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?\s*$/;

function table(rows: string[]): string {
  const head = cells(rows[0]);
  const align = cells(rows[1]).map((c) => (c.startsWith(":") && c.endsWith(":") ? "center" : c.endsWith(":") ? "right" : ""));
  const td = (tag: string, c: string, i: number) => `<${tag}${align[i] ? ` style="text-align:${align[i]}"` : ""}>${inline(c)}</${tag}>`;
  const body = rows
    .slice(2)
    .map((r) => {
      const cs = cells(r);
      return `<tr>${head.map((_, i) => td("td", cs[i] ?? "", i)).join("")}</tr>`;
    })
    .join("");
  return `<div class="tbl"><table><thead><tr>${head.map((c, i) => td("th", c, i)).join("")}</tr></thead><tbody>${body}</tbody></table></div>`;
}

export function markdown(src: string): string {
  const lines = escapeHtml(src.replace(/\r/g, "")).split("\n");
  let html = "";
  let list: "ul" | "ol" | null = null;
  let para: string[] = [];
  let quote: string[] = [];
  let code: string[] | null = null;

  const flushPara = () => {
    if (para.length) html += `<p>${para.map(inline).join("<br>")}</p>`;
    para = [];
  };
  const flushList = () => {
    if (list) html += `</${list}>`;
    list = null;
  };
  const flushQuote = () => {
    if (quote.length) html += `<blockquote>${quote.map(inline).join("<br>")}</blockquote>`;
    quote = [];
  };
  const flushAll = () => {
    flushPara();
    flushList();
    flushQuote();
  };

  for (let n = 0; n < lines.length; n++) {
    const line = lines[n];
    if (code) {
      if (/^\s*```/.test(line)) {
        html += `<pre><code>${code.join("\n")}</code></pre>`;
        code = null;
      } else code.push(line);
      continue;
    }
    if (/^\s*```/.test(line)) {
      flushAll();
      code = [];
      continue;
    }
    // Tabella: riga con le barre seguita dalla riga di separazione «|---|---|»
    if (line.includes("|") && n + 1 < lines.length && SEPARATOR.test(lines[n + 1]) && lines[n + 1].includes("-")) {
      flushAll();
      const rows = [line, lines[n + 1]];
      n += 2;
      while (n < lines.length && lines[n].includes("|") && lines[n].trim()) rows.push(lines[n++]);
      n--;
      html += table(rows);
      continue;
    }
    const ul = line.match(/^\s*[-*•]\s+(.*)$/);
    const ol = line.match(/^\s*\d+[.)]\s+(.*)$/);
    if (ul || ol) {
      flushPara();
      flushQuote();
      const kind = ul ? "ul" : "ol";
      if (list !== kind) {
        flushList();
        html += `<${kind}>`;
        list = kind;
      }
      html += `<li>${inline((ul ?? ol)![1])}</li>`;
      continue;
    }
    const q = line.match(/^\s*&gt;\s?(.*)$/);
    if (q) {
      flushPara();
      flushList();
      quote.push(q[1]);
      continue;
    }
    const h = line.match(/^\s*#{1,6}\s+(.*)$/);
    if (h) {
      flushAll();
      html += `<h4>${inline(h[1])}</h4>`;
      continue;
    }
    if (/^\s*(---|\*\*\*)\s*$/.test(line)) {
      flushAll();
      html += "<hr>";
      continue;
    }
    if (!line.trim()) {
      flushAll();
      continue;
    }
    flushList();
    flushQuote();
    para.push(line);
  }
  if (code) html += `<pre><code>${(code as string[]).join("\n")}</code></pre>`;
  flushAll();
  return html;
}
