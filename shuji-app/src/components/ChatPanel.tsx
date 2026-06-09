import { useState } from 'react';
import { cancelProcessing } from '../api';
import { useLatestDeptLogs } from '../hooks/useLatestDeptLogs';
import { getDeptMeta } from '../constants';
import ChatBubble from './ChatBubble';
import ChatInput from './ChatInput';
import type { ChatMessage, DeptLogEntry, PlanInfo } from '../types';

interface ChatPanelProps {
  tab: 'decision' | 'discuss';
  messages: ChatMessage[];
  discussMsgs: ChatMessage[];
  discussing: boolean;
  planInfo: PlanInfo | null;
  activeDeptsCount: number;
  onOption: (key: string, supplement?: string) => void;
  onSend: (text: string) => void;
  onRetrySend: (text: string, ts: string) => void;
  onDiscuss: (text: string) => void;
  onCancelDiscuss?: () => void;
  onConvertToCommand: (text: string) => void;
  endRef: React.RefObject<HTMLDivElement | null>;
}

export default function ChatPanel(props: ChatPanelProps) {
  const {
    tab,
    messages,
    discussMsgs,
    discussing,
    planInfo,
    activeDeptsCount,
    onOption,
    onSend,
    onRetrySend,
    onDiscuss,
    onCancelDiscuss,
    onConvertToCommand,
    endRef,
  } = props;
  const latestLogs = useLatestDeptLogs();
  const [toast, setToast] = useState('');
  const isProcessing = activeDeptsCount > 0;

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(''), 2500);
  };

  const handleCancel = async () => {
    try {
      await cancelProcessing();
      showToast('已叫停诸司');
    } catch {
      showToast('叫停失败');
    }
  };

  if (tab === 'decision') {
    return (
      <>
        {toast && (
          <div className="shrink-0 -mb-px text-center text-xs text-gold bg-gold-light/60 rounded-b px-3 py-1.5">
            {toast}
          </div>
        )}
        {planInfo && <PlanCard info={planInfo} />}
        <MessageList
          messages={messages}
          onOption={onOption}
          onRetry={onRetrySend}
          endRef={endRef}
          thinking={isProcessing}
          thinkingLabel="诸司处理中…"
          latestLogs={latestLogs}
        />
        <div className="shrink-0 px-4 py-2 border-t border-fold bg-surface-elevated flex justify-end">
          <button
            onClick={handleCancel}
            className="text-ui px-3 py-1.5 font-medium text-ink-600 hover:text-vermillion hover:bg-vermillion-light rounded-lg transition-colors"
          >
            叫停诸司
          </button>
        </div>
        <ChatInput
          onSend={onSend}
          disabled={isProcessing}
          placeholder={isProcessing ? '诸司处理中…' : '拟旨…'}
        />
      </>
    );
  }
  return (
    <>
      {discussing && onCancelDiscuss && (
        <div className="shrink-0 px-4 py-2 border-t border-fold bg-surface-elevated flex justify-end">
          <button
            onClick={onCancelDiscuss}
            className="text-ui px-3 py-1.5 font-medium text-vermillion hover:bg-vermillion-light rounded-lg transition-colors"
          >
            叫停讨论
          </button>
        </div>
      )}
      <MessageList
        messages={discussMsgs}
        onOption={() => {}}
        endRef={endRef}
        thinking={discussing}
      />
      {discussMsgs.length > 0 && !discussing && (
        <div className="shrink-0 px-4 py-2 border-t border-fold bg-surface-elevated">
          <button
            onClick={() => {
              const lastUserMsg = [...discussMsgs]
                .reverse()
                .find((m) => m.role === 'user' || m.role === '皇帝');
              if (lastUserMsg) onConvertToCommand(lastUserMsg.content);
            }}
            className="w-full px-3 py-2 text-ui font-medium text-gold bg-gold-light hover:bg-gold-light/80 border border-vermillion/20 rounded-lg transition-colors"
          >
            将此事转为正式敕命
          </button>
        </div>
      )}
      <ChatInput onSend={onDiscuss} disabled={discussing} placeholder="廷议…" />
    </>
  );
}

function MessageList({
  messages,
  onOption,
  onRetry,
  endRef,
  thinking,
  thinkingLabel,
  latestLogs,
}: {
  messages: ChatMessage[];
  onOption: (key: string, supplement?: string) => void;
  onRetry?: (text: string, ts: string) => void;
  endRef: React.RefObject<HTMLDivElement | null>;
  thinking?: boolean;
  thinkingLabel?: string;
  latestLogs?: Map<string, DeptLogEntry>;
}) {
  return (
    <div className="flex-1 overflow-y-auto p-4 space-y-2 min-h-0">
      {messages.map((msg, i) => (
        <ChatBubble key={messageKey(msg, i)} msg={msg} onOption={onOption} onRetry={onRetry} />
      ))}
      {thinking && (
        <div className="flex flex-col items-center gap-1.5 py-3">
          <div className="flex items-center gap-2">
            <span className="w-2 h-2 rounded-full bg-gold animate-pulse" />
            <span className="text-xs text-ink-500 font-medium">
              {thinkingLabel || '内阁思考中…'}
            </span>
          </div>
          {latestLogs && latestLogs.size > 0 && (
            <div className="flex flex-wrap justify-center gap-x-3 gap-y-1 max-w-full">
              {Array.from(latestLogs.entries()).map(([dept, entry]) => (
                <LiveAction key={dept} dept={dept} entry={entry} />
              ))}
            </div>
          )}
        </div>
      )}
      <div ref={endRef} />
    </div>
  );
}

function messageKey(msg: ChatMessage, index: number) {
  return `${msg.timestamp || index}|${msg.role}|${msg.content.slice(0, 40)}`;
}

/** Live per-department action indicator shown in the thinking area */
function LiveAction({ dept, entry }: { dept: string; entry: DeptLogEntry }) {
  const meta = getDeptMeta(dept);
  const color = meta?.color || '#8B7355';
  const label = meta?.shortLabel || dept;
  const action = entry.action.replace(/^[❌→]\s*/, '').replace(/:.*/, '');
  const target = entry.action.includes(': ') ? entry.action.split(': ').pop() : '';
  return (
    <div
      className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded text-[10px] font-mono"
      style={{ backgroundColor: `${color}12` }}
    >
      <span
        className="w-1.5 h-1.5 rounded-full animate-pulse shrink-0"
        style={{ backgroundColor: color }}
      />
      <span className="font-semibold text-ink-600">{label}</span>
      <span className="text-ink-400 truncate max-w-[140px]">{action}</span>
      {target && (
        <span className="text-ink-300 truncate max-w-[100px] hidden sm:inline">{target}</span>
      )}
    </div>
  );
}

function PlanCard({ info }: { info: PlanInfo }) {
  return (
    <div className="shrink-0 mx-4 mt-3 bg-surface-parchment border border-fold rounded-lg px-3 py-2">
      <div className="font-display text-caption text-ink-600 font-semibold mb-1">工部计划</div>
      <div className="space-y-0.5">
        {info.batches.map((b, i) => (
          <div key={i} className="flex items-center gap-1.5 text-caption font-mono">
            <span
              className={`w-1.5 h-1.5 rounded-full shrink-0 ${b.status === 'done' ? 'bg-jade' : b.status === 'current' ? 'bg-gold animate-pulse' : 'bg-ink-300'}`}
            />
            <span
              className={
                b.status === 'done'
                  ? 'text-ink-400 line-through'
                  : b.status === 'current'
                    ? 'text-ink-800 font-medium'
                    : 'text-ink-500'
              }
            >
              {b.name}
            </span>
            {b.status === 'current' && (
              <span className="text-ink-400 text-caption ml-auto truncate">{b.goal}</span>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
