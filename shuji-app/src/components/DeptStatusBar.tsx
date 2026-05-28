import { useEffect, useState } from "react";
import { getTokenStats } from "../api";
import { useActiveDepts } from "../hooks/useActiveDepts";

export const DEPT_COLORS: Record<string, string> = {
  内阁: "#7c3aed",
  中书令: "#3b82f6",
  门下侍中: "#06b6d4",
  尚书令: "#f97316",
  吏部: "#22c55e",
  兵部: "#ef4444",
  工部: "#f59e0b",
  刑部: "#6b7280",
  礼部: "#6366f1",
  户部: "#14b8a6",
  制司: "#ec4899",
};

const ROLE_ORDER = ["内阁", "中书令", "门下侍中", "尚书令", "吏部", "兵部", "工部", "刑部", "礼部", "户部", "制司"];

export default function DeptStatusBar() {
  const active = useActiveDepts();
  const [tokenTotal, setTokenTotal] = useState(0);

  useEffect(() => {
    const load = () => {
      getTokenStats().then((stats) => {
        const total = Object.values(stats["汇总"] || {}).reduce((sum, u) => sum + u.total_tokens, 0);
        setTokenTotal(total);
      }).catch((e) => console.error("Token统计加载失败:", e));
    };
    load();
    const timer = window.setInterval(load, 30000);
    return () => window.clearInterval(timer);
  }, []);

  const visible = ROLE_ORDER.slice(0, 10);
  const hidden = ROLE_ORDER.slice(10);

  return (
    <div className="h-6 bg-ink-900 text-ink-300 border-t border-ink-800 px-3 flex items-center justify-between text-[11px] shrink-0">
      <div className="flex items-center gap-2 min-w-0">
        <span className="text-amber-300 shrink-0">⚡</span>
        {visible.map((dept) => <DeptLight key={dept} dept={dept} active={active.has(dept)} />)}
        {hidden.length > 0 && (
          <span className="group relative text-ink-500 cursor-default">
            +{hidden.length} 更多
            <span className="hidden group-hover:flex absolute bottom-5 left-0 bg-ink-800 border border-ink-700 rounded px-2 py-1 gap-2 whitespace-nowrap shadow-lg">
              {hidden.map((dept) => <DeptLight key={dept} dept={dept} active={active.has(dept)} />)}
            </span>
          </span>
        )}
      </div>
      <div className="font-mono text-ink-400 shrink-0 ml-3">token {formatToken(tokenTotal)}</div>
    </div>
  );
}

function DeptLight({ dept, active }: { dept: string; active: boolean }) {
  const color = DEPT_COLORS[dept] || "#9ca3af";
  return (
    <span className="flex items-center gap-0.5 whitespace-nowrap">
      <span>{dept}</span>
      <span className={active ? "animate-pulse" : ""} style={{ color }}>{active ? "●" : "○"}</span>
    </span>
  );
}

function formatToken(n: number) {
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
  return String(n);
}
