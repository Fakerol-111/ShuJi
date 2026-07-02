import { getDocumentDiffs, readDocumentDiff } from '../../api';
import type { DocumentDiff } from '../../api';

export function countPatchStats(patch: string): { added: number; removed: number } {
  let added = 0;
  let removed = 0;
  for (const line of patch.split('\n')) {
    if (line.startsWith('+') && !line.startsWith('+++')) added++;
    else if (line.startsWith('-') && !line.startsWith('---')) removed++;
  }
  return { added, removed };
}

export async function loadAuditDiff(docId: string): Promise<DocumentDiff | null> {
  const diffs = await getDocumentDiffs(docId);
  if (diffs.length === 0) return null;
  const latest = diffs.reduce((a, b) => (a.ts >= b.ts ? a : b));
  const patch = await readDocumentDiff(latest.filename);
  if (!patch.trim()) return null;
  const { added, removed } = countPatchStats(patch);
  return { diff: patch, has_previous: true, added, removed };
}
