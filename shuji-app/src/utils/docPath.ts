const DIR_BY_PREFIX: Record<string, string> = {
  reqs: 'requirements',
  dsgn: 'designs',
  plan: 'designs',
  pdsg: 'designs',
  ddtl: 'designs/detail',
  revw: 'reviews',
  task: 'tasks',
  ctrt: 'contracts',
  rprt: 'reports',
  anls: 'analysis',
};

export function docIdToPath(id: string): string {
  if (id.startsWith('.shuji/')) return id;
  const prefix = id.split('_')[0];
  const dir = DIR_BY_PREFIX[prefix];
  if (dir) return `.shuji/${dir}/${id}.md`;
  return `.shuji/designs/${id}.md`;
}
