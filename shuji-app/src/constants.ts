// ── UI 行为 ───────────────────────────────────────────────

export const DEPT_ACTIVE_TIMEOUT_MS = 5000;
export const MAX_LOG_ENTRIES = 300;
export const CHAT_PANEL_DEFAULT_WIDTH = 400;
export const CHAT_PANEL_MIN_WIDTH = 300;
export const CHAT_PANEL_MAX_WIDTH = 600;
export const TOKEN_REFRESH_INTERVAL_MS = 30000;

// ── Token 预警 ────────────────────────────────────────────

export const MONTHLY_TOKEN_WARNING = 0.8; // 80% 时黄色预警
export const MONTHLY_TOKEN_LIMIT = 10_000_000; // 1000 万 tokens

// ── 代码主题 ────────────────────────────────────────────

export interface CodeTheme {
  label: string;
  type: "dark" | "light";
  bg: string;
  tabBg: string;
  border: string;
  text: string;
  lineNum: string;
  muted: string;
  lineHover: string;
}

export const CODE_THEMES: Record<string, CodeTheme> = {
  "tokyo-night": {
    label: "Tokyo Night",
    type: "dark",
    bg: "#1a1b26",
    tabBg: "#16161e",
    border: "#2f3b54",
    text: "#c0caf5",
    lineNum: "#4c4f6b",
    muted: "#565f89",
    lineHover: "rgba(255,255,255,0.03)",
  },
  "github-dark": {
    label: "GitHub Dark",
    type: "dark",
    bg: "#0d1117",
    tabBg: "#161b22",
    border: "#30363d",
    text: "#e6edf3",
    lineNum: "#6e7681",
    muted: "#8b949e",
    lineHover: "rgba(255,255,255,0.03)",
  },
  "one-light": {
    label: "One Light",
    type: "light",
    bg: "#fafafa",
    tabBg: "#f0f0f0",
    border: "#e0e0e0",
    text: "#383a42",
    lineNum: "#9d9d9f",
    muted: "#a0a1a7",
    lineHover: "rgba(0,0,0,0.03)",
  },
  "github-light": {
    label: "GitHub Light",
    type: "light",
    bg: "#ffffff",
    tabBg: "#f6f8fa",
    border: "#d0d7de",
    text: "#1f2328",
    lineNum: "#6e7781",
    muted: "#656d76",
    lineHover: "rgba(0,0,0,0.03)",
  },
};

export const DEFAULT_CODE_THEME = "tokyo-night";

export function getCodeTheme(): string {
  if (typeof window === "undefined") return DEFAULT_CODE_THEME;
  return localStorage.getItem("shuji_code_theme") || DEFAULT_CODE_THEME;
}

export function setCodeTheme(name: string): void {
  localStorage.setItem("shuji_code_theme", name);
}

// ── 代码主题 end ────────────────────────────────────────

// ── Department metadata (single source of truth) ──────────

export interface DeptMeta {
  key: string;         // English key matching backend Role enum: "neige", "zhongshuling", ...
  label: string;       // Chinese full name: "内阁", "中书令", ...
  shortLabel: string;  // Chinese short name for narrow spaces
  description: string; // One-line role description
  color: string;       // Hex accent color for borders & dots
  bg: string;          // Tailwind bg class
  text: string;        // Tailwind text class
  accent: string;      // Tailwind border-l class for left accent bar
}

export const DEPT_META_LIST: DeptMeta[] = [
  { key: "neige",           label: "内阁",     shortLabel: "内阁",   description: "奏折整理", color: "#6B4E9E", bg: "bg-purple-50",  text: "text-purple-700",  accent: "border-l-purple-400" },
  { key: "zhongshuling",    label: "中书令",   shortLabel: "中书",   description: "方案设计", color: "#3D6B8E", bg: "bg-blue-50",    text: "text-blue-700",    accent: "border-l-blue-400" },
  { key: "menxiashizhong",  label: "门下侍中", shortLabel: "门下",   description: "审查",     color: "#2E7D8C", bg: "bg-cyan-50",    text: "text-cyan-700",    accent: "border-l-cyan-400" },
  { key: "shangshuling",    label: "尚书令",   shortLabel: "尚书令", description: "执行管理", color: "#B45309", bg: "bg-orange-50",  text: "text-orange-700",  accent: "border-l-orange-400" },
  { key: "libushangshu",    label: "吏部尚书", shortLabel: "吏部",   description: "任务拆解", color: "#2F7A4F", bg: "bg-green-50",   text: "text-green-700",   accent: "border-l-green-400" },
  { key: "bingbushangshu",  label: "兵部尚书", shortLabel: "兵部",   description: "测试",     color: "#B83A3A", bg: "bg-red-50",     text: "text-red-700",     accent: "border-l-red-400" },
  { key: "gongbushangshu",  label: "工部尚书", shortLabel: "工部",   description: "编码实现", color: "#A16207", bg: "bg-amber-50",   text: "text-amber-700",   accent: "border-l-amber-400" },
  { key: "xingbushangshu",  label: "刑部尚书", shortLabel: "刑部",   description: "异常检查", color: "#5C6370", bg: "bg-gray-50",    text: "text-gray-600",    accent: "border-l-gray-400" },
  { key: "liburshangshu",   label: "礼部尚书", shortLabel: "礼部",   description: "规范检查", color: "#5B5FC7", bg: "bg-indigo-50",  text: "text-indigo-700",  accent: "border-l-indigo-400" },
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
  zhongshu: "zhongshuling",
  menxia: "menxiashizhong",
  shangshu: "shangshuling",
  libup: "libushangshu",
  bingbu: "bingbushangshu",
  gongbu: "gongbushangshu",
  xingbu: "xingbushangshu",
  libur: "liburshangshu",
  hubu: "hubu",
};

/** Resolve any role key (Chinese, full English, abbreviated English) to DeptMeta */
export function getDeptMeta(key: string): DeptMeta | undefined {
  // Try Chinese label first
  if (DEPT_META[key]) return DEPT_META[key];
  // Try full English key
  if (DEPT_META_BY_KEY[key]) return DEPT_META_BY_KEY[key];
  // Try abbreviated alias
  const resolved = DEPT_KEY_ALIASES[key];
  if (resolved) return DEPT_META_BY_KEY[resolved];
  return undefined;
}

/** Display order for departments (matches backend Role enum order) */
export const DEPT_ORDER = DEPT_META_LIST.map((d) => d.label);

/** Convenience: English key → Chinese label */
export const DEPT_LABEL_BY_KEY: Record<string, string> = Object.fromEntries(
  DEPT_META_LIST.map((d) => [d.key, d.label])
);

// ── Legacy RoleInfo (used by SetupPage) ──────────────────

export interface RoleInfo {
  key: string;
  label: string;
  description: string;
}

export const ALL_ROLES: RoleInfo[] = [
  { key: "default", label: "默认（全局）", description: "所有未单独配置的角色使用此回退" },
  ...DEPT_META_LIST.map((d) => ({ key: d.key, label: d.label, description: d.description })),
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
  { token_threshold: number; keep_recent_count: number; mid_run_compact: boolean }
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
