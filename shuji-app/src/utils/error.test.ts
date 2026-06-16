import { describe, it, expect } from 'vitest';
import { formatError, classifyError } from './error';

describe('formatError', () => {
  it('maps 401 to API key invalid message', () => {
    expect(formatError('401 unauthorized')).toBe('API 密钥无效或已过期');
    expect(formatError(new Error('Invalid API key'))).toBe(
      'API 密钥无效或已过期'
    );
  });

  it('maps 403 to forbidden message', () => {
    expect(formatError('403 Forbidden')).toBe('API 访问被拒绝，请检查密钥权限');
  });

  it('maps 429 to rate limit message', () => {
    expect(formatError('429 rate limit exceeded')).toBe('API 请求过于频繁');
  });

  it('maps timeout errors', () => {
    expect(formatError('Request timed out')).toBe('API 请求超时，请稍后重试或检查网络');
  });

  it('maps connection errors', () => {
    expect(formatError('Connection refused')).toBe(
      '无法连接 API 服务器'
    );
    expect(formatError('tcp connect error')).toBe('无法连接 API 服务器');
  });

  it('maps 5xx server errors', () => {
    expect(formatError('500 Internal Server Error')).toBe('API 服务器内部错误，请稍后重试');
    expect(formatError('502 Bad Gateway')).toBe('API 服务暂时不可用，请稍后重试');
    expect(formatError('503 Service Unavailable')).toBe('API 服务暂时不可用，请稍后重试');
  });

  it('maps 400 bad request', () => {
    expect(formatError('400 bad request')).toBe('请求参数错误，请检查输入');
  });

  it('maps 404 not found', () => {
    expect(formatError('404 not found')).toBe('API 端点不存在，请检查 API URL');
  });

  it('truncates very long raw errors', () => {
    const long = 'x'.repeat(300);
    const result = formatError(long);
    expect(result).toMatch(/^系统错误: /);
    expect(result.length).toBeLessThan(220);
  });

  it('wraps unknown errors with 系统错误', () => {
    expect(formatError('something random')).toBe('系统错误: something random');
  });

  it('handles non-string, non-Error inputs', () => {
    // JSON.stringify fallback
    const result = formatError({ foo: 'bar' });
    expect(result).toContain('系统错误');
  });

  it('returns 未知错误 for empty input', () => {
    expect(formatError('')).toBe('未知错误');
    expect(formatError('  ')).toBe('未知错误');
  });

  it('handles Error objects', () => {
    expect(formatError(new Error('429 too many'))).toBe('API 请求过于频繁');
  });
});

describe('classifyError', () => {
  it('classifies API/auth errors as critical', () => {
    expect(classifyError('API error')).toBe('critical');
    expect(classifyError('密钥错误')).toBe('critical');
    expect(classifyError('auth failure')).toBe('critical');
  });

  it('classifies timeout/network errors as warning', () => {
    expect(classifyError('request timeout')).toBe('warning');
    expect(classifyError('network error')).toBe('warning');
  });

  it('classifies other errors as info', () => {
    expect(classifyError('file not found')).toBe('info');
    expect(classifyError('')).toBe('info');
  });
});
