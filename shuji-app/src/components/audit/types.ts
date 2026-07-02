import {
  TimelineData,
  LineageNode,
  TraceResult,
  DocSummary,
  DocumentLineRun,
  LineNode,
} from '../../types';
import type { VerificationReport } from '../../api';

export type SubTab =
  | 'timeline'
  | 'lineage'
  | 'trace'
  | 'docline'
  | 'report'
  | 'dashboard'
  | 'search';

export interface TabProps {
  t: (key: string, options?: Record<string, unknown>) => string;
  onDocSelect?: (path: string) => void;
  onShowDiff?: (path: string) => void;
}

export interface TimelineTabProps extends TabProps {
  data: TimelineData | null;
  loading: boolean;
  error: string;
  searchText: string;
  onSearchTextChange: (v: string) => void;
}

export interface LineageTabProps extends TabProps {
  lineageDocId: string;
  onChangeLineageDocId: (v: string) => void;
  lineage: LineageNode | null;
  lineageLoading: boolean;
  onSearch: () => void;
}

export interface TraceTabProps extends TabProps {
  traceDocId: string;
  onChangeTraceDocId: (v: string) => void;
  traceResult: TraceResult | null;
  traceLoading: boolean;
  onTrace: () => void;
  onJumpToDocLine: (docId: string) => void;
}

export interface DocumentLineTabProps extends TabProps {
  docLineRuns: string[];
  docLineRunId: string;
  onChangeDocLineRunId: (v: string) => void;
  docLineDocId: string;
  onChangeDocLineDocId: (v: string) => void;
  docLine: DocumentLineRun | null;
  docLineLoading: boolean;
  onLoadDocLine: (runId?: string, focusDocId?: string) => void;
  onDocLineNodeClick: (node: LineNode) => void;
}

export interface SearchTabProps extends TabProps {
  searchStatus: string;
  onChangeSearchStatus: (v: string) => void;
  searchKeyword: string;
  onChangeSearchKeyword: (v: string) => void;
  searchResults: DocSummary[];
  searchLoading: boolean;
  onSearch: () => void;
  onQuickFilter: (status: string) => void;
}

export interface ReportTabProps extends TabProps {
  report: string | null;
  reportLoading: boolean;
  onLoadReport: () => void;
}

export interface DashboardTabProps extends TabProps {
  data: TimelineData | null;
  loading: boolean;
  verification: VerificationReport | null;
  verifying: boolean;
  onVerifyTrail: () => void;
}

export const EVENT_COLORS: Record<string, string> = {
  create_document: 'text-jade',
  modify_document: 'text-azure',
  append_document: 'text-azure',
  set_document_status: 'text-gold',
  cancel_agent: 'text-vermillion',
  checkpoint: 'text-info',
  milestone: 'text-ink-500',
};

export const NODE_KIND_COLORS: Record<string, string> = {
  document: 'border-jade/40 bg-jade/5',
  pipeline_step: 'border-azure/40 bg-azure/5',
  approval: 'border-gold/40 bg-gold/5',
  diff: 'border-ink-300 bg-ink-50',
  validation: 'border-jade/40 bg-jade/10',
  checkpoint: 'border-info/40 bg-info/5',
};
