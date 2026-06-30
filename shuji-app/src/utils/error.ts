/**
 * Frontend error formatting — wraps raw errors into user-friendly messages.
 * Use in catch blocks instead of `String(e)` or `e.message`.
 */

import i18n from '../i18n/config';

/** Common API error messages mapped from keywords */
const API_ERROR_MAP: Array<[RegExp, string]> = [
  [/401|unauthorized|invalid api key/i, 'error.apiKeyInvalid'],
  [/403|forbidden/i, 'error.apiForbidden'],
  [/429|rate limit/i, 'error.rateLimited'],
  [/timeout|timed out/i, 'error.timeout'],
  [/connection refused|connect error|tcp connect/i, 'error.connectionFailed'],
  [/500|internal server error/i, 'error.serverError'],
  [/502|503|service unavailable/i, 'error.serviceUnavailable'],
  [/400|bad request/i, 'error.badRequest'],
  [/404|api endpoint.*not found|endpoint.*not found|not found.*api url/i, 'error.notFound'],
  [/api error \(unknown\)/i, 'error.unknownApiError'],
];

/** Map a raw error (string or Error) to a user-friendly message */
export function formatError(e: unknown): string {
  const raw = typeof e === 'string' ? e : e instanceof Error ? e.message : String(e);
  const msg = raw.trim();
  if (!msg) return i18n.t('error.unknownError');

  // Try keyword matching
  for (const [pattern, key] of API_ERROR_MAP) {
    if (pattern.test(msg)) return i18n.t(key);
  }

  // Truncate very long raw errors
  if (msg.length > 200) {
    return `${i18n.t('error.systemError')} ${msg.slice(0, 200)}…`;
  }
  return `${i18n.t('error.systemError')} ${msg}`;
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

/**
 * Create a swallow-error handler for fire-and-forget promise chains.
 *
 * Use in `.catch(swallowError('contextName'))` instead of `.catch(() => {})`
 * to preserve debug logs without breaking fire-and-forget semantics.
 *
 * @example
 *   cancelDiscussApi().catch(swallowError('cancelDiscuss'));
 *   getReasoningConfig().then(setReasoningConfig).catch(swallowError('loadReasoningConfig'));
 */
export function swallowError(context: string): (e: unknown) => void {
  return (e: unknown) => {
    const msg = typeof e === 'string' ? e : e instanceof Error ? e.message : String(e);
    console.error(`[${context}]`, msg, e);
  };
}
