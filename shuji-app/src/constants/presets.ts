/**
 * Shared API provider presets — single source of truth for SettingsMenu and SetupPage.
 */

export const API_URL_PRESETS = [
  { label: 'DeepSeek', url: 'https://api.deepseek.com/chat/completions' },
  { label: 'Anthropic', url: 'https://api.anthropic.com/v1/messages' },
  { label: 'OpenAI', url: 'https://api.openai.com/v1/chat/completions' },
  { label: '自定义', url: '' },
] as const;

export const MODEL_PRESETS: Record<string, string[]> = {
  'https://api.deepseek.com/chat/completions': ['deepseek-v4-flash', 'deepseek-4-pro'],
  'https://api.anthropic.com/v1/messages': [
    'claude-sonnet-4-20250514',
    'claude-haiku-4-5-20251001',
  ],
  'https://api.openai.com/v1/chat/completions': ['gpt-4o', 'gpt-4o-mini'],
};

export function detectProvider(url: string): string {
  if (url.includes('anthropic.com')) return 'Anthropic';
  if (url.includes('deepseek.com')) return 'DeepSeek';
  if (url.includes('openai.com')) return 'OpenAI';
  return '自定义';
}
