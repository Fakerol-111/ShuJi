/**
 * Frontend error formatting — wraps raw errors into user-friendly Chinese messages.
 * Use in catch blocks instead of `String(e)` or `e.message`.
 */

/** Common Chinese API error messages mapped from keywords */
const API_ERROR_MAP: Array<[RegExp, string]> = [
  [/401|unauthorized|invalid api key/i, 'API 密钥无效或已过期，请在设置中重新配置'],
  [/403|forbidden/i, 'API 访问被拒绝，请检查密钥权限'],
  [/429|rate limit/i, 'API 请求过于频繁，请稍后重试'],
  [/timeout|timed out/i, 'API 请求超时，请稍后重试或检查网络'],
  [/connection refused|connect error|tcp connect/i, '无法连接 API 服务器，请检查网络或 API URL 配置'],
  [/500|internal server error/i, 'API 服务器内部错误，请稍后重试'],
  [/502|503|service unavailable/i, 'API 服务暂时不可用，请稍后重试'],
  [/400|bad request/i, '请求参数错误，请检查输入'],
  [/404|not found/i, 'API 端点不存在，请检查 API URL'],
  [/api error \(unknown\)/i, 'API 返回未知错误，请稍后重试'],
];

/** Map a raw error (string or Error) to a Chinese user-friendly message */
export function formatError(e: unknown): string {
  const raw = typeof e === 'string' ? e : e instanceof Error ? e.message : String(e);
  const msg = raw.trim();
  if (!msg) return '未知错误';

  // Try keyword matching
  for (const [pattern, hint] of API_ERROR_MAP) {
    if (pattern.test(msg)) return hint;
  }

  // Truncate very long raw errors
  if (msg.length > 200) {
    return `系统错误: ${msg.slice(0, 200)}…`;
  }
  return `系统错误: ${msg}`;
}

/** Error severity categories for UI treatment */
export type ErrorSeverity = 'critical' | 'warning' | 'info';

/** Determine severity based on error content */
export function classifyError(e: unknown): ErrorSeverity {
  const raw = typeof e === 'string' ? e : e instanceof Error ? e.message : String(e);
  const msg = raw.toLowerCase();
  if (msg.includes('api') || msg.includes('密钥') || msg.includes('auth')) return 'critical';
  if (msg.includes('timeout') || msg.includes('network')) return 'warning';
  return 'info';
}
