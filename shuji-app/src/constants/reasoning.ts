import type { ReasoningEffort } from '../types';

export const EFFORT_LABELS: Record<
  ReasoningEffort,
  { zh: string; en: string; desc: string; descEn: string }
> = {
  none: {
    zh: '关闭',
    en: 'Off',
    desc: '不发送思考字段，成本最低',
    descEn: 'No thinking field, lowest cost',
  },
  low: {
    zh: '轻量',
    en: 'Low',
    desc: '轻量推理，适合验证、扫描、闲聊',
    descEn: 'Light reasoning, for verification, scanning, chat',
  },
  medium: {
    zh: '平衡',
    en: 'Medium',
    desc: '平衡模式，适合一般设计与执行调度',
    descEn: 'Balanced, for general design and execution',
  },
  high: {
    zh: '深度',
    en: 'High',
    desc: '深度推理，适合内阁规划、中书设计、工部批次规划',
    descEn: 'Deep reasoning, for cabinet planning, design, batch planning',
  },
};

export const EFFORT_ORDER: ReasoningEffort[] = ['none', 'low', 'medium', 'high'];

/** Built-in default effort per role (matches Rust `builtin_role_reasoning`) */
export const ROLE_BUILTIN_EFFORT: Record<string, ReasoningEffort> = {
  内阁: 'high',
  中书令: 'high',
  门下侍中: 'medium',
  尚书令: 'medium',
  吏部尚书: 'medium',
  兵部尚书: 'medium',
  工部尚书: 'medium',
  刑部尚书: 'low',
  礼部尚书: 'low',
};
