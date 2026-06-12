import { Tabs } from './ui/Tabs';
import ChatPanel from './ChatPanel';
import type { Project, PlanInfo, ChatMessage } from '../types';
import type { Tab } from '../hooks/useChat';

interface Props {
  chatWidth: number;
  project: Project | null;
  tab: Tab;
  messages: ChatMessage[];
  discussMsgs: ChatMessage[];
  discussing: boolean;
  planInfo: PlanInfo | null;
  activeDeptsCount: number;
  setTab: (tab: Tab) => void;
  onOption: (key: string, supplement?: string) => void;
  onSend: (text: string) => void;
  onRetrySend: (text: string, originalTs: string) => Promise<void>;
  onDiscuss: (text: string) => void;
  onCancelDiscuss: () => void;
  onConvertToCommand: (text: string) => void;
  endRef: React.RefObject<HTMLDivElement | null>;
  onResizeStart: (e: React.MouseEvent) => void;
}

export default function DashboardChatPanel({
  chatWidth,
  project,
  tab,
  messages,
  discussMsgs,
  discussing,
  planInfo,
  activeDeptsCount,
  setTab,
  onOption,
  onSend,
  onRetrySend,
  onDiscuss,
  onCancelDiscuss,
  onConvertToCommand,
  endRef,
  onResizeStart,
}: Props) {
  return (
    <section
      className="relative bg-surface-paper border-l border-fold flex flex-col min-h-0 shrink-0"
      style={{ width: chatWidth }}
    >
      <div
        onMouseDown={onResizeStart}
        className="absolute left-0 top-0 bottom-0 w-1 cursor-col-resize hover:bg-vermillion/40 transition-colors"
      />
      <div className="border-b border-fold bg-surface-elevated shrink-0 px-3 py-2">
        <Tabs
          tabs={[
            { key: 'decision', label: '决策' },
            { key: 'discuss', label: '廷议' },
          ]}
          activeKey={tab}
          onChange={(k) => setTab(k as Tab)}
        />
        <div className="text-ui text-ink-600 mt-1">
          {tab === 'decision' ? '下达敕令，驱动各部门执行' : '仅与内阁议政，不改代码、不写文档'}
        </div>
      </div>
      {!project ? (
        <div className="flex-1 flex items-center justify-center text-body text-ink-400">
          请先开卷
        </div>
      ) : (
        <ChatPanel
          tab={tab}
          messages={messages}
          discussMsgs={discussMsgs}
          discussing={discussing}
          planInfo={planInfo}
          activeDeptsCount={activeDeptsCount}
          onOption={(key, supplement) => onOption(key, supplement)}
          onSend={onSend}
          onRetrySend={onRetrySend}
          onDiscuss={onDiscuss}
          onCancelDiscuss={onCancelDiscuss}
          onConvertToCommand={onConvertToCommand}
          endRef={endRef}
        />
      )}
    </section>
  );
}
