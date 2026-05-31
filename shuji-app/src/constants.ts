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

// ── Role definitions ──────────────────────────────────────

export interface RoleInfo {
  key: string;
  label: string;
  description: string;
}

export const ALL_ROLES: RoleInfo[] = [
  { key: "default", label: "默认（全局）", description: "所有未单独配置的角色使用此回退" },
  { key: "menxiashizhong", label: "门下侍中", description: "审查" },
  { key: "zhongshuling", label: "中书令", description: "方案设计" },
  { key: "neige", label: "内阁", description: "奏折整理" },
  { key: "shangshuling", label: "尚书令", description: "执行管理" },
  { key: "libushangshu", label: "吏部", description: "任务拆解" },
  { key: "liburshangshu", label: "礼部", description: "规范检查" },
  { key: "bingbushangshu", label: "兵部", description: "测试" },
  { key: "xingbushangshu", label: "刑部", description: "异常检查" },
  { key: "gongbushangshu", label: "工部", description: "编码实现" },
  { key: "zhisi", label: "制司", description: "独立调查" },
];

/** 各部门内置上下文压缩默认值（与 Rust `default_compact_thresholds_for_role` 一致） */
export const ROLE_CONTEXT_DEFAULTS: Record<
  string,
  { char_threshold: number; keep_recent_count: number; history_char_threshold: number; mid_run_compact: boolean }
> = {
  工部: { char_threshold: 120_000, keep_recent_count: 16, history_char_threshold: 3_500, mid_run_compact: true },
  刑部: { char_threshold: 110_000, keep_recent_count: 14, history_char_threshold: 3_500, mid_run_compact: true },
  中书令: { char_threshold: 100_000, keep_recent_count: 12, history_char_threshold: 3_000, mid_run_compact: true },
  吏部: { char_threshold: 100_000, keep_recent_count: 12, history_char_threshold: 3_000, mid_run_compact: true },
  内阁: { char_threshold: 100_000, keep_recent_count: 14, history_char_threshold: 4_000, mid_run_compact: true },
  兵部: { char_threshold: 85_000, keep_recent_count: 10, history_char_threshold: 2_500, mid_run_compact: true },
  门下侍中: { char_threshold: 70_000, keep_recent_count: 8, history_char_threshold: 2_000, mid_run_compact: true },
  制司: { char_threshold: 65_000, keep_recent_count: 8, history_char_threshold: 2_000, mid_run_compact: true },
  尚书令: { char_threshold: 55_000, keep_recent_count: 6, history_char_threshold: 1_500, mid_run_compact: true },
  礼部: { char_threshold: 50_000, keep_recent_count: 6, history_char_threshold: 1_500, mid_run_compact: true },
};
