// Markdown leggero per le risposte: paragrafi, elenchi, titoli, grassetto, corsivo,
// codice in linea e a blocchi, citazioni, link http(s). Tutto il resto resta testo.
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
    .replace(/(^|[^*\w])\*(?!\s)(.+?)\*(?!\w)/g, "$1<em>$2</em>")
    .replace(/\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g, '<a href="$2" target="_blank" rel="noreferrer">$1</a>')
    .replace(/(^|[\s(])(https?:\/\/[^\s<)]+)/g, '$1<a href="$2" target="_blank" rel="noreferrer">$2</a>');
  return out.replace(/\u0000(\d+)\u0000/g, (_, i) => `<code>${codes[Number(i)]}</code>`);
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

  for (const line of lines) {
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
