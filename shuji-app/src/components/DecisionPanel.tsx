import { useState } from "react";
import type { Document } from "../types";

interface Props {
  documents: Document[];
  onDecision: (choice: string, comment?: string) => void;
  disabled?: boolean;
}

const choices = [
  { key: "A", label: "准", description: "批准执行", color: "bg-green-600 hover:bg-green-700" },
  { key: "B", label: "准，但", description: "批准方向，需微调", color: "bg-blue-600 hover:bg-blue-700" },
  { key: "C", label: "驳", description: "方案不可行，重新设计", color: "bg-red-600 hover:bg-red-700" },
  { key: "D", label: "暂缓", description: "方向对但时机不对", color: "bg-yellow-600 hover:bg-yellow-700" },
  { key: "E", label: "钦此", description: "皇帝另有想法", color: "bg-purple-600 hover:bg-purple-700" },
];

export default function DecisionPanel({ documents, onDecision, disabled }: Props) {
  const [comment, setComment] = useState("");

  return (
    <div className="bg-amber-50 border border-amber-300 rounded-lg p-4">
      <h3 className="text-lg font-bold text-amber-900 mb-2">⚡ 需要皇帝御批</h3>

      {documents.length > 0 && (
        <div className="mb-3 text-sm text-amber-800">
          {documents.map((doc, i) => (
            <p key={i} className="mb-1">
              <strong>{doc.doc_type}：</strong>
              {doc.title}
            </p>
          ))}
        </div>
      )}

      <div className="grid grid-cols-5 gap-2 mb-3">
        {choices.map((c) => (
          <button
            key={c.key}
            onClick={() => onDecision(c.key, comment || undefined)}
            disabled={disabled}
            className={`${c.color} text-white font-bold py-3 px-2 rounded-lg transition disabled:opacity-50`}
            title={c.description}
          >
            <div className="text-lg">{c.label}</div>
            <div className="text-xs opacity-80">{c.description}</div>
          </button>
        ))}
      </div>

      <div>
        <input
          type="text"
          placeholder="御批备注（可选）..."
          value={comment}
          onChange={(e) => setComment(e.target.value)}
          disabled={disabled}
          className="w-full px-3 py-2 border border-amber-300 rounded bg-white text-sm"
        />
      </div>
    </div>
  );
}
