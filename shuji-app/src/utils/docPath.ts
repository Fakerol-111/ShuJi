export function docIdToPath(id: string): string {
  if (id.startsWith('.shuji/')) return id;
  const prefix = id.split('_')[0];
  if (prefix === 'revw') return `.shuji/reviews/${id}.md`;
  if (prefix === 'plan') return `.shuji/plans/${id}.md`;
  return `.shuji/designs/${id}.md`;
}
