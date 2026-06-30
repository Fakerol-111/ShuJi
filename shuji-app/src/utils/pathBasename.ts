/**
 * Extract the last path segment, handling both `/` and `\` separators.
 */
export function basenameFromPath(path: string): string {
  const normalized = path.replace(/\\/g, '/');
  const parts = normalized.split('/');
  return parts.pop() || path;
}

/**
 * Split a path into non-empty segments, normalizing `\` to `/`.
 */
export function splitPathParts(path: string): string[] {
  return path.replace(/\\/g, '/').split('/').filter(Boolean);
}
