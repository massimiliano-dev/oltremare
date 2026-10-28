// Allegati: icone per tipo e nomi brevi.
export function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

export function fileIcon(name: string): string {
  const ext = name.split(".").pop()?.toLowerCase() ?? "";
  if (ext === "pdf") return "book";
  if (["png", "jpg", "jpeg", "gif", "webp"].includes(ext)) return "image";
  if (["xlsx", "xlsm", "xlsb", "xls", "ods", "csv", "tsv"].includes(ext)) return "layers";
  if (["docx", "odt", "pptx", "txt", "md", "rtf"].includes(ext)) return "draft";
  return "clip";
}

export function fileExt(name: string): string {
  return (name.split(".").pop() ?? "").toUpperCase().slice(0, 4);
}
