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
