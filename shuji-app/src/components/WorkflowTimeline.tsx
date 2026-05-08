import type { PhaseSnapshot } from "../types";

interface Props {
  overallProgress: number;
  phases: PhaseSnapshot[];
}

function statusColor(label: string): string {
  if (label.includes("已完成") || label.includes("已批准")) return "bg-green-500";
  if (label.includes("未开始")) return "bg-gray-300";
  if (label.includes("驳回") || label.includes("阻塞")) return "bg-red-500";
  if (label.includes("待批") || label.includes("决策")) return "bg-amber-500";
  return "bg-blue-500";
}

export default function WorkflowTimeline({ overallProgress, phases }: Props) {
  return (
    <div className="bg-white rounded-lg border p-4">
      <h3 className="font-bold text-gray-700 mb-3">流程进度</h3>

      {/* Progress bar */}
      <div className="mb-4">
        <div className="flex justify-between text-sm text-gray-500 mb-1">
          <span>整体进度</span>
          <span>{overallProgress.toFixed(0)}%</span>
        </div>
        <div className="w-full bg-gray-200 rounded-full h-3">
          <div
            className="bg-blue-600 h-3 rounded-full transition-all duration-500"
            style={{ width: `${Math.min(overallProgress, 100)}%` }}
          />
        </div>
      </div>

      {/* Phase cards */}
      <div className="space-y-2">
        {phases.map((phase) => (
          <div key={phase.index} className="border rounded p-3 text-sm">
            <div className="font-bold text-gray-700 mb-1">阶段 {phase.index}</div>
            <div className="flex items-center gap-2 mb-1">
              <span className="text-gray-500 w-16">设计：</span>
              <span className={`px-2 py-0.5 rounded text-white text-xs ${statusColor(phase.design)}`}>
                {phase.design}
              </span>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-gray-500 w-16">执行：</span>
              <span className={`px-2 py-0.5 rounded text-white text-xs ${statusColor(phase.execution)}`}>
                {phase.execution}
              </span>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
