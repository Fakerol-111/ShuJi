// ── UI 行为 ───────────────────────────────────────────────

export const DEPT_ACTIVE_TIMEOUT_MS = 5000;
export const MAX_LOG_ENTRIES = 300;
export const CHAT_PANEL_DEFAULT_WIDTH = 400;
export const CHAT_PANEL_MIN_WIDTH = 300;
export const CHAT_PANEL_MAX_WIDTH = 600;
export const TOKEN_REFRESH_INTERVAL_MS = 30000;

// ── Token 预警 ────────────────────────────────────────────

// (已清理: MONTHLY_TOKEN_WARNING / LIMIT 无 UI 引用，TokenPanel 直接使用 API 数据)

// ── 代码主题 ────────────────────────────────────────────

export interface CodeTheme {
  label: string;
  type: 'dark' | 'light';
  bg: string;
  tabBg: string;
  border: string;
  text: string;
  lineNum: string;
  muted: string;
  lineHover: string;
}

export const CODE_THEMES: Record<string, CodeTheme> = {
  'tokyo-night': {
    label: 'Tokyo Night',
    type: 'dark',
    bg: '#1a1b26',
    tabBg: '#16161e',
    border: '#2f3b54',
    text: '#c0caf5',
    lineNum: '#4c4f6b',
    muted: '#565f89',
    lineHover: 'rgba(255,255,255,0.03)',
  },
  'github-dark': {
    label: 'GitHub Dark',
    type: 'dark',
    bg: '#0d1117',
    tabBg: '#161b22',
    border: '#30363d',
    text: '#e6edf3',
    lineNum: '#6e7681',
    muted: '#8b949e',
    lineHover: 'rgba(255,255,255,0.03)',
  },
  'vscode-dark': {
    label: 'VS Code Dark',
    type: 'dark',
    bg: '#1e1e1e',
    tabBg: '#252526',
    border: '#3c3c3c',
    text: '#d4d4d4',
    lineNum: '#858585',
    muted: '#9b9b9b',
    lineHover: 'rgba(255,255,255,0.04)',
  },
  gruvbox: {
    label: 'Gruvbox',
    type: 'dark',
    bg: '#282828',
    tabBg: '#1d2021',
    border: '#504945',
    text: '#ebdbb2',
    lineNum: '#7c6f64',
    muted: '#928374',
    lineHover: 'rgba(235,219,178,0.06)',
  },
  dracula: {
    label: 'Dracula',
    type: 'dark',
    bg: '#282a36',
    tabBg: '#21222c',
    border: '#44475a',
    text: '#f8f8f2',
    lineNum: '#6272a4',
    muted: '#8be9fd',
    lineHover: 'rgba(255,255,255,0.04)',
  },
  nord: {
    label: 'Nord',
    type: 'dark',
    bg: '#2e3440',
    tabBg: '#242933',
    border: '#434c5e',
    text: '#d8dee9',
    lineNum: '#4c566a',
    muted: '#81a1c1',
    lineHover: 'rgba(216,222,233,0.05)',
  },
  monokai: {
    label: 'Monokai',
    type: 'dark',
    bg: '#272822',
    tabBg: '#1f201b',
    border: '#49483e',
    text: '#f8f8f2',
    lineNum: '#75715e',
    muted: '#a59f85',
    lineHover: 'rgba(248,248,242,0.05)',
  },
  'one-light': {
    label: 'One Light',
    type: 'light',
    bg: '#fafafa',
    tabBg: '#f0f0f0',
    border: '#e0e0e0',
    text: '#383a42',
    lineNum: '#9d9d9f',
    muted: '#a0a1a7',
    lineHover: 'rgba(0,0,0,0.03)',
  },
  'github-light': {
    label: 'GitHub Light',
    type: 'light',
    bg: '#ffffff',
    tabBg: '#f6f8fa',
    border: '#d0d7de',
    text: '#1f2328',
    lineNum: '#6e7781',
    muted: '#656d76',
    lineHover: 'rgba(0,0,0,0.03)',
  },
  'solarized-light': {
    label: 'Solarized Light',
    type: 'light',
    bg: '#fdf6e3',
    tabBg: '#eee8d5',
    border: '#d6cfb8',
    text: '#657b83',
    lineNum: '#93a1a1',
    muted: '#839496',
    lineHover: 'rgba(101,123,131,0.08)',
  },
  parchment: {
    label: 'Parchment',
    type: 'light',
    bg: '#fbf7ef',
    tabBg: '#f1e7d6',
    border: '#d8c8aa',
    text: '#3d3328',
    lineNum: '#a8926d',
    muted: '#8b7355',
    lineHover: 'rgba(92,79,62,0.06)',
  },
};

export const DEFAULT_CODE_THEME = 'tokyo-night';

export function getCodeTheme(): string {
  if (typeof window === 'undefined') return DEFAULT_CODE_THEME;
  return localStorage.getItem('shuji_code_theme') || DEFAULT_CODE_THEME;
}

export function setCodeTheme(name: string): void {
  localStorage.setItem('shuji_code_theme', name);
}

// ── 代码主题 end ────────────────────────────────────────

// ── 字体大小 ────────────────────────────────────────────

export interface FontSizeTier {
  label: string; // 显示名称，如 "密集"
  description: string; // 说明，如 "13px — 信息密度高"
  labelEn: string; // English display name
  descriptionEn: string; // English description
  value: number; // rem 基准 px 值
}

export const FONT_SIZE_TIERS: Record<string, FontSizeTier> = {
  dense: {
    label: '密集',
    description: '13px — 信息密度高',
    labelEn: 'Dense',
    descriptionEn: '13px — high information density',
    value: 13,
  },
  compact: {
    label: '紧凑',
    description: '14px — 适中紧凑',
    labelEn: 'Compact',
    descriptionEn: '14px — moderately compact',
    value: 14,
  },
  relaxed: {
    label: '舒缓',
    description: '15px — 柔和舒适',
    labelEn: 'Relaxed',
    descriptionEn: '15px — soft and comfortable',
    value: 15,
  },
};

export const DEFAULT_FONT_SIZE = 'relaxed';

export function getFontSize(): string {
  if (typeof window === 'undefined') return DEFAULT_FONT_SIZE;
  return localStorage.getItem('shuji_font_size') || DEFAULT_FONT_SIZE;
}

export function setFontSize(tier: string): void {
  localStorage.setItem('shuji_font_size', tier);
}

// ── 字体大小 end ────────────────────────────────────────

// ── Department metadata (single source of truth) ──────────

export interface DeptMeta {
  key: string; // English key matching backend Role enum: "neige", "zhongshuling", ...
  label: string; // Chinese full name: "内阁", "中书令", ...
  shortLabel: string; // Chinese short name for narrow spaces
  description: string; // One-line role description
  labelEn: string; // English display name: "Cabinet", "Architect", ...
  shortLabelEn: string; // English short name
  descriptionEn: string; // English description
  color: string; // Hex accent color for borders & dots
  bg: string; // Tailwind bg class
  text: string; // Tailwind text class
  accent: string; // Tailwind border-l class for left accent bar
  tintClass?: string; // dept-tint-{key} utility class
  tintActiveClass?: string; // dept-tint-active-{key} utility class
}

export const DEPT_META_LIST: DeptMeta[] = [
  {
    key: 'neige',
    label: '内阁',
    shortLabel: '内阁',
    description: '奏折整理',
    labelEn: 'Cabinet',
    shortLabelEn: 'Cabinet',
    descriptionEn: 'Orchestrator — workflow dispatch & coordination',
    color: '#6B4E9E',
    bg: 'bg-purple-50',
    text: 'text-purple-700',
    accent: 'border-l-purple-400',
    tintClass: 'dept-tint-neige',
    tintActiveClass: 'dept-tint-active-neige',
  },
  {
    key: 'zhongshuling',
    label: '中书令',
    shortLabel: '中书',
    description: '方案设计',
    labelEn: 'Architect',
    shortLabelEn: 'Architect',
    descriptionEn: 'Design — architecture & solution design',
    color: '#3D6B8E',
    bg: 'bg-blue-50',
    text: 'text-blue-700',
    accent: 'border-l-blue-400',
    tintClass: 'dept-tint-zhongshuling',
    tintActiveClass: 'dept-tint-active-zhongshuling',
  },
  {
    key: 'menxiashizhong',
    label: '门下侍中',
    shortLabel: '门下',
    description: '审查',
    labelEn: 'Reviewer',
    shortLabelEn: 'Reviewer',
    descriptionEn: 'Review — design review & quality gate',
    color: '#2E7D8C',
    bg: 'bg-cyan-50',
    text: 'text-cyan-700',
    accent: 'border-l-cyan-400',
    tintClass: 'dept-tint-menxiashizhong',
    tintActiveClass: 'dept-tint-active-menxiashizhong',
  },
  {
    key: 'shangshuling',
    label: '尚书令',
    shortLabel: '尚书令',
    description: '执行管理',
    labelEn: 'Executor',
    shortLabelEn: 'Executor',
    descriptionEn: 'Execution — task scheduling & orchestration',
    color: '#B45309',
    bg: 'bg-orange-50',
    text: 'text-orange-700',
    accent: 'border-l-orange-400',
    tintClass: 'dept-tint-shangshuling',
    tintActiveClass: 'dept-tint-active-shangshuling',
  },
  {
    key: 'libushangshu',
    label: '吏部尚书',
    shortLabel: '吏部',
    description: '任务拆解',
    labelEn: 'Designer',
    shortLabelEn: 'Designer',
    descriptionEn: 'Detailed Design — task breakdown & specs',
    color: '#2F7A4F',
    bg: 'bg-green-50',
    text: 'text-green-700',
    accent: 'border-l-green-400',
    tintClass: 'dept-tint-libushangshu',
    tintActiveClass: 'dept-tint-active-libushangshu',
  },
  {
    key: 'bingbushangshu',
    label: '兵部尚书',
    shortLabel: '兵部',
    description: '测试',
    labelEn: 'Tester',
    shortLabelEn: 'Tester',
    descriptionEn: 'Testing — test plans & interface contracts',
    color: '#B83A3A',
    bg: 'bg-red-50',
    text: 'text-red-700',
    accent: 'border-l-red-400',
    tintClass: 'dept-tint-bingbushangshu',
    tintActiveClass: 'dept-tint-active-bingbushangshu',
  },
  {
    key: 'gongbushangshu',
    label: '工部尚书',
    shortLabel: '工部',
    description: '编码实现',
    labelEn: 'Developer',
    shortLabelEn: 'Developer',
    descriptionEn: 'Implementation — TDD coding',
    color: '#A16207',
    bg: 'bg-amber-50',
    text: 'text-amber-700',
    accent: 'border-l-amber-400',
    tintClass: 'dept-tint-gongbushangshu',
    tintActiveClass: 'dept-tint-active-gongbushangshu',
  },
  {
    key: 'xingbushangshu',
    label: '刑部尚书',
    shortLabel: '刑部',
    description: '异常检查',
    labelEn: 'Validator',
    shortLabelEn: 'Validator',
    descriptionEn: 'Validation — integration testing & bug filing',
    color: '#5C6370',
    bg: 'bg-gray-50',
    text: 'text-gray-600',
    accent: 'border-l-gray-400',
    tintClass: 'dept-tint-xingbushangshu',
    tintActiveClass: 'dept-tint-active-xingbushangshu',
  },
  {
    key: 'liburshangshu',
    label: '礼部尚书',
    shortLabel: '礼部',
    description: '规范检查',
    labelEn: 'Auditor',
    shortLabelEn: 'Auditor',
    descriptionEn: 'Standards — code quality & audit',
    color: '#5B5FC7',
    bg: 'bg-indigo-50',
    text: 'text-indigo-700',
    accent: 'border-l-indigo-400',
    tintClass: 'dept-tint-liburshangshu',
    tintActiveClass: 'dept-tint-active-liburshangshu',
  },
];

/** Primary lookup by Chinese label (used in ChatMessage.role, activeDepts, etc.) */
export const DEPT_META: Record<string, DeptMeta> = Object.fromEntries(
  DEPT_META_LIST.map((d) => [d.label, d])
);

/** Lookup by English key (used in backend API responses) */
export const DEPT_META_BY_KEY: Record<string, DeptMeta> = Object.fromEntries(
  DEPT_META_LIST.map((d) => [d.key, d])
);

/** Abbreviated English key aliases (used in some legacy API responses like token stats) */
const DEPT_KEY_ALIASES: Record<string, string> = {
  zhongshu: 'zhongshuling',
  menxia: 'menxiashizhong',
  shangshu: 'shangshuling',
  libup: 'libushangshu',
  bingbu: 'bingbushangshu',
  gongbu: 'gongbushangshu',
  xingbu: 'xingbushangshu',
  libur: 'liburshangshu',
  hubu: 'hubu',
};

/** Short-label → long-label mapping (backend Role::name() uses short names for 尚书 roles) */
const DEPT_SHORT_TO_LONG: Record<string, string> = Object.fromEntries(
  DEPT_META_LIST.map((d) => [d.shortLabel, d.label])
);

/** Resolve any role key (Chinese long, Chinese short, full English, abbreviated English) to DeptMeta */
export function getDeptMeta(key: string): DeptMeta | undefined {
  // Try Chinese long label first
  if (DEPT_META[key]) return DEPT_META[key];
  // Try Chinese short label
  if (DEPT_SHORT_TO_LONG[key]) return DEPT_META[DEPT_SHORT_TO_LONG[key]];
  // Try full English key
  if (DEPT_META_BY_KEY[key]) return DEPT_META_BY_KEY[key];
  // Try case-insensitive English key (backend sends "Zhongshuling", keys are lowercase)
  const lowerKey = key.toLowerCase();
  if (DEPT_META_BY_KEY[lowerKey]) return DEPT_META_BY_KEY[lowerKey];
  // Try abbreviated alias
  const resolved = DEPT_KEY_ALIASES[key];
  if (resolved) return DEPT_META_BY_KEY[resolved];
  return undefined;
}

/** Bilingual dept label: EN shows role + Chinese; ZH shows Chinese short name. */
export function getDeptDisplayLabel(meta: DeptMeta, lang: 'en' | 'zh'): string {
  if (lang === 'en') {
    return `${meta.shortLabelEn} · ${meta.shortLabel}`;
  }
  return meta.shortLabel;
}

/** Display order for departments (matches backend Role enum order) */
export const DEPT_ORDER = DEPT_META_LIST.map((d) => d.label);

/** 驾驶舱轨道分组 — 规划层 / 执行层 */
export const DEPT_RAIL_GROUPS: {
  title: string;
  subtitle: string;
  titleEn: string;
  subtitleEn: string;
  labels: string[];
}[] = [
  {
    title: '规划',
    subtitle: '三省',
    titleEn: 'Planning',
    subtitleEn: 'Three Departments',
    labels: ['内阁', '中书令', '门下侍中', '尚书令'],
  },
  {
    title: '执行',
    subtitle: '六部',
    titleEn: 'Execution',
    subtitleEn: 'Six Ministries',
    labels: ['吏部尚书', '兵部尚书', '工部尚书', '刑部尚书', '礼部尚书'],
  },
];

/** Convenience: English key → Chinese label */
export const DEPT_LABEL_BY_KEY: Record<string, string> = Object.fromEntries(
  DEPT_META_LIST.map((d) => [d.key, d.label])
);

/** Get department label in the specified language */
export function deptLabel(d: DeptMeta, lang: string): string {
  return lang === 'en' ? d.labelEn : d.label;
}

/** Get department short label in the specified language */
export function deptShortLabel(d: DeptMeta, lang: string): string {
  return lang === 'en' ? d.shortLabelEn : d.shortLabel;
}

/** Get department description in the specified language */
export function deptDescription(d: DeptMeta, lang: string): string {
  return lang === 'en' ? d.descriptionEn : d.description;
}

/** Get font size tier label in the specified language */
export function fontSizeLabel(tier: FontSizeTier, lang: string): string {
  return lang === 'en' ? tier.labelEn : tier.label;
}

/** Get font size tier description in the specified language */
export function fontSizeDescription(tier: FontSizeTier, lang: string): string {
  return lang === 'en' ? tier.descriptionEn : tier.description;
}

// ── Legacy RoleInfo (used by SetupPage) ──────────────────

export interface RoleInfo {
  key: string;
  label: string;
  description: string;
  labelEn?: string;
  descriptionEn?: string;
}

export const ALL_ROLES: RoleInfo[] = [
  {
    key: 'default',
    label: '默认（全局）',
    description: '所有未单独配置的角色使用此回退',
    labelEn: 'Default (Global)',
    descriptionEn: 'All roles without individual config use this fallback',
  },
  ...DEPT_META_LIST.map((d) => ({
    key: d.key,
    label: d.label,
    description: d.description,
    labelEn: d.labelEn,
    descriptionEn: d.descriptionEn,
  })),
];

/** DeepSeek 1M 窗口：接近上限再压缩（cl100k token 计数，各部门统一） */
const NEAR_WINDOW_CONTEXT = {
  token_threshold: 750_000,
  keep_recent_count: 24,
  mid_run_compact: false,
} as const;

/** 各部门内置上下文压缩默认值（与 Rust `default_compact_thresholds_for_role` 一致） */
export const ROLE_CONTEXT_DEFAULTS: Record<
  string,
  {
    token_threshold: number;
    keep_recent_count: number;
    mid_run_compact: boolean;
  }
> = {
  工部: { ...NEAR_WINDOW_CONTEXT },
  刑部: { ...NEAR_WINDOW_CONTEXT },
  中书令: { ...NEAR_WINDOW_CONTEXT },
  吏部: { ...NEAR_WINDOW_CONTEXT },
  内阁: { ...NEAR_WINDOW_CONTEXT },
  兵部: { ...NEAR_WINDOW_CONTEXT },
  门下侍中: { ...NEAR_WINDOW_CONTEXT },
  尚书令: { ...NEAR_WINDOW_CONTEXT },
  礼部: { ...NEAR_WINDOW_CONTEXT },
};
