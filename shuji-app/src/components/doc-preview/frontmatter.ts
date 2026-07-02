import { basenameFromPath } from '../../utils/pathBasename';

export function parseFrontmatter(raw: string): {
  meta: Record<string, string> | null;
  body: string;
} {
  const match = raw.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?/);
  if (!match) return { meta: null, body: raw };
  const header = match[1];
  const body = raw.slice(match[0].length).trimStart();
  const meta: Record<string, string> = {};
  for (const line of header.split(/\r?\n/)) {
    const idx = line.indexOf(':');
    if (idx > 0) meta[line.slice(0, idx).trim()] = line.slice(idx + 1).trim();
  }
  return { meta, body };
}

export function docIdFromPath(docPath: string, metaId?: string): string {
  if (metaId) return metaId;
  return basenameFromPath(docPath).replace(/\.md$/, '') || '';
}
