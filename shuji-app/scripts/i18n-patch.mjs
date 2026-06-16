import { readFileSync, writeFileSync, existsSync } from 'fs';

const PROJECT = 'D:/ProgrammeProject/ShuJi/shuji-app';

// Map of file paths to replacements: [search, replace] pairs
// Each replacement is applied in order to the file content
const patches = new Map();

// Helper to register a patch for a file
function addPatch(file, search, replace) {
  if (!patches.has(file)) patches.set(file, []);
  patches.get(file).push([search, replace]);
}

// ── SetupPage ──
addPatch('src/pages/SetupPage.tsx',
  `              <label className="block text-ui font-medium text-ink-600 mb-1.5">服务商</label>`,
  `              <label className="block text-ui font-medium text-ink-600 mb-1.5">{t('setup.provider')}</label>`
);
addPatch('src/pages/SetupPage.tsx',
  `              <label className="block text-ui font-medium text-ink-600 mb-1.5">模型</label>`,
  `              <label className="block text-ui font-medium text-ink-600 mb-1.5">{t('setup.model')}</label>`
);
addPatch('src/pages/SetupPage.tsx',
  `                {testing ? '测试中...' : '测试连接'}`,
  `                {testing ? t('setup.testing') : t('setup.testConnection')}`
);
addPatch('src/pages/SetupPage.tsx',
  `              {testResult === 'ok' && <span className="text-ui text-jade">✔ 连接成功</span>}`,
  `              {testResult === 'ok' && <span className="text-ui text-jade">{t('setup.connectionSuccess')}</span>}`
);
addPatch('src/pages/SetupPage.tsx',
  `                <span className="text-ui text-vermillion">✘ 连接失败，请检查配置</span>`,
  `                <span className="text-ui text-vermillion">{t('setup.connectionFailHint')}</span>`
);
addPatch('src/pages/SetupPage.tsx',
  `                {showAdvanced ? '▾ 高级配置' : '▸ 高级配置'}`,
  `                {showAdvanced ? '▾ ' + t('setup.advancedConfig') : '▸ ' + t('setup.advancedConfig')}`
);
addPatch('src/pages/SetupPage.tsx',
  `                  <p>
                    进入主界面后点击右上角 <strong>设置</strong>，可为各部门分别配置 API。
                  </p>
                  <p>当前默认 key 将被所有部门共享，除非在设置中单独覆盖。</p>
                  <p>模型分级预设只影响角色 model 字段，不改 API URL/Key。</p>`,
  `                  <p>{t('setup.advancedHint1')}</p>
                  <p>{t('setup.advancedHint2')}</p>
                  <p>{t('setup.advancedHint3')}</p>`
);
addPatch('src/pages/SetupPage.tsx',
  `              <Button variant="ghost" className="flex-1" onClick={() => setStep(1)}>
                上一步
              </Button>
              <Button variant="primary" className="flex-1" disabled={saving} onClick={handleSave}>
                {saving ? '保存中...' : '保存并继续'}
              </Button>`,
  `              <Button variant="ghost" className="flex-1" onClick={() => setStep(1)}>
                {t('setup.back')}
              </Button>
              <Button variant="primary" className="flex-1" disabled={saving} onClick={handleSave}>
                {saving ? t('setup.saving') : t('setup.saveAndContinue')}
              </Button>`
);

// Step 3
addPatch('src/pages/SetupPage.tsx',
  `            <h2 className="font-display text-sm font-bold text-ink-900 text-center">
              各部门配置概览
            </h2>
            <p className="text-caption text-ink-500 text-center">
              以下是根据你的预设自动分配的模型。后续可在设置中为各部门单独配置。
            </p>`,
  `            <h2 className="font-display text-sm font-bold text-ink-900 text-center">
              {t('setup.roleOverviewTitle')}
            </h2>
            <p className="text-caption text-ink-500 text-center">
              {t('setup.roleOverviewDesc')}
            </p>`
);
addPatch('src/pages/SetupPage.tsx',
  `              <Button variant="ghost" className="flex-1" onClick={() => setStep(2)}>
                上一步
              </Button>
              <Button variant="primary" className="flex-1" onClick={() => setStep(4)}>
                确认并完成
              </Button>`,
  `              <Button variant="ghost" className="flex-1" onClick={() => setStep(2)}>
                {t('setup.back')}
              </Button>
              <Button variant="primary" className="flex-1" onClick={() => setStep(4)}>
                {t('setup.confirmAndFinish')}
              </Button>`
);

// Step 4
addPatch('src/pages/SetupPage.tsx',
  `              <h2 className="font-display text-sm font-bold text-ink-900">配置已保存！</h2>
              <p className="text-body text-ink-600 mt-1">现在可以开始使用枢机了</p>`,
  `              <h2 className="font-display text-sm font-bold text-ink-900">{t('setup.configSaved')}</h2>
              <p className="text-body text-ink-600 mt-1">{t('setup.configSavedDesc')}</p>`
);
addPatch('src/pages/SetupPage.tsx',
  `                🚀 开始第一个项目`,
  `                {t('setup.startFirstProject')}`
);
addPatch('src/pages/SetupPage.tsx',
  `                ⚡ 先跑一个 Demo`,
  `                {t('setup.runDemo')}`
);
addPatch('src/pages/SetupPage.tsx',
  `                返回首页`,
  `                {t('setup.returnHome')}`
);
addPatch('src/pages/SetupPage.tsx',
  `      setError('请输入 API 密钥');`,
  `      setError(t('setup.apiKeyRequired'));`
);

// ── SettingsPage ──
addPatch('src/pages/SettingsPage.tsx',
  `            ← {onClose ? '关闭' : '返回项目'}`,
  `            ← {onClose ? t('common.close') : t('settings.backToProject')}`
);
addPatch('src/pages/SettingsPage.tsx',
  `          <h1 className="font-display text-base font-semibold text-ink-50">枢机 · 设置</h1>`,
  `          <h1 className="font-display text-base font-semibold text-ink-50">{t('settings.title')}</h1>`
);
addPatch('src/pages/SettingsPage.tsx',
  `          <SettingsSaveButton onClick={handleSave}>保存所有更改</SettingsSaveButton>`,
  `          <SettingsSaveButton onClick={handleSave}>{t('common.saveAll')}</SettingsSaveButton>`
);
addPatch('src/pages/SettingsPage.tsx',
  `      setSavedMsg('已保存');`,
  `      setSavedMsg(t('common.saved'));`
);
addPatch('src/pages/SettingsPage.tsx',
  `  const CATEGORY_LABELS: Record<SettingsCategory, string> = {
    service: '服务配置',
    context: '上下文窗口',
    soul: '灵魂管理',
    appearance: '外观',
  };`,
  `  const CATEGORY_LABELS: Record<SettingsCategory, string> = {
    service: t('settings.serviceConfig'),
    context: t('settings.contextWindow'),
    soul: t('settings.soulManagement'),
    appearance: t('settings.appearance'),
  };`
);
addPatch('src/pages/SettingsPage.tsx',
  `    if (healthStatus === 'checking')
      return <span className="text-xs text-ink-300">⏳ 探测 API 连接中...</span>;
    if (healthStatus === 'ok') return <span className="text-xs text-jade-light">✔ 连接成功</span>;
    return <span className="text-xs text-vermillion-light">✘ 连接失败: {healthMsg}</span>;`,
  `    if (healthStatus === 'checking')
      return <span className="text-xs text-ink-300">{t('common.loading')}</span>;
    if (healthStatus === 'ok') return <span className="text-xs text-jade-light">{t('setup.connectionSuccess')}</span>;
    return <span className="text-xs text-vermillion-light">{t('setup.connectionFailed')}: {healthMsg}</span>;`
);

// ── Sidebar ──
addPatch('src/components/Sidebar.tsx',
  `const headerLabel: Record<string, string> = {
  files: '架阁目录',
  stats: '度支',
  context: '文脉',
  archives: '存档',
  audit: '朝报',
};`,
  `const headerLabel: Record<string, string> = {
  files: 'sidebar.directory',
  stats: 'sidebar.tokens',
  context: 'sidebar.context',
  archives: 'sidebar.checkpoints',
  audit: 'sidebar.audit',
};`
);
addPatch('src/components/Sidebar.tsx',
  `  return headerLabel[mode] || mode;`,
  `  const { t } = useTranslation();
  const key = headerLabel[mode] || mode;
  return t(key, key);`
);

// ── DutyBar ──
addPatch('src/components/DutyBar.tsx',
  `import { useEffect, useState } from 'react';
import { getTokenStats, getRoundMetrics } from '../api';
import { useDeptEvents } from '../hooks/useDeptEvents';
import { getDeptMeta, DEPT_META_LIST } from '../constants';
import DeptStatusPanel from './DeptStatusPanel';`,
  `import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { getTokenStats, getRoundMetrics } from '../api';
import { useDeptEvents } from '../hooks/useDeptEvents';
import { getDeptMeta, DEPT_META_LIST } from '../constants';
import DeptStatusPanel from './DeptStatusPanel';`
);
addPatch('src/components/DutyBar.tsx',
  `export default function DutyBar() {
  const [logsExpanded, setLogsExpanded] = useState(false);
  const [tokenExpanded, setTokenExpanded] = useState(false);`,
  `export default function DutyBar() {
  const { t, i18n } = useTranslation();
  const isEn = i18n.language?.startsWith('en');
  const [logsExpanded, setLogsExpanded] = useState(false);
  const [tokenExpanded, setTokenExpanded] = useState(false);`
);
addPatch('src/components/DutyBar.tsx',
  `          <span className="text-gold/60 text-caption font-serif font-semibold tracking-wider mr-0.5">
            值事
          </span>
          {deptArray.length === 0 ? (
            <span className="text-ink-500 italic text-caption">诸司无事</span>`,
  `          <span className="text-gold/60 text-caption font-serif font-semibold tracking-wider mr-0.5">
            {t('duty.title')}
          </span>
          {deptArray.length === 0 ? (
            <span className="text-ink-500 italic text-caption">{t('activityBar.allQuiet')}</span>`
);
addPatch('src/components/DutyBar.tsx',
  `              <span className="text-gold/60">输出</span>`,
  `              <span className="text-gold/60">{t('duty.output')}</span>`
);
addPatch('src/components/DutyBar.tsx',
  `          className="ml-2 pl-2 border-l border-ink-800 flex items-center gap-1 text-caption text-ink-500 hover:text-ink-300 font-mono shrink-0"
          title="度支明细"
        >
          <span>{tokenExpanded ? '▾' : '▸'}</span>
          度支`,
  `          className="ml-2 pl-2 border-l border-ink-800 flex items-center gap-1 text-caption text-ink-500 hover:text-ink-300 font-mono shrink-0"
          title={t('duty.tokens')}
        >
          <span>{tokenExpanded ? '▾' : '▸'}</span>
          {t('duty.tokens')}`
);
addPatch('src/components/DutyBar.tsx',
  `          <span>{logsExpanded ? '▾' : '▸'}</span>
          日志`,
  `          <span>{logsExpanded ? '▾' : '▸'}</span>
          {t('duty.logs')}`
);
addPatch('src/components/DutyBar.tsx',
  `            <span className="text-jade/80">输入缓存命中</span>
            <span className="text-ink-300">{formatToken(tokenCached)}</span>
            <span className="text-ink-700">|</span>
            <span className="text-ink-400">输入缓存未命中</span>
            <span className="text-ink-300">{formatToken(tokenPrompt - tokenCached)}</span>
            <span className="text-ink-700">|</span>
            <span className="text-gold/60">输出</span>`,
  `            <span className="text-jade/80">{t('duty.cacheHit')}</span>
            <span className="text-ink-300">{formatToken(tokenCached)}</span>
            <span className="text-ink-700">|</span>
            <span className="text-ink-400">{t('duty.inputCacheHit')}</span>
            <span className="text-ink-300">{formatToken(tokenPrompt - tokenCached)}</span>
            <span className="text-ink-700">|</span>
            <span className="text-gold/60">{t('duty.output')}</span>`
);
addPatch('src/components/DutyBar.tsx',
  `                  title="切换货币"`,
  `                  title={t('common.refresh')}`
);

// ── ChatPanel ──
addPatch('src/components/ChatPanel.tsx',
  `import { useState } from 'react';
import { cancelProcessing } from '../api';
import ChatBubble from './ChatBubble';
import ChatInput from './ChatInput';
import DeptGlyph from './DeptGlyph';
import type { ChatMessage, PlanInfo } from '../types';
import type { ChatInputHandle } from './ChatInput';
import { useDeptEvents } from '../hooks/useDeptEvents';`,
  `import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cancelProcessing } from '../api';
import ChatBubble from './ChatBubble';
import ChatInput from './ChatInput';
import DeptGlyph from './DeptGlyph';
import type { ChatMessage, PlanInfo } from '../types';
import type { ChatInputHandle } from './ChatInput';
import { useDeptEvents } from '../hooks/useDeptEvents';`
);
addPatch('src/components/ChatPanel.tsx',
  `  const [toast, setToast] = useState('');
  const { activeDepts } = useDeptEvents();`,
  `  const { t } = useTranslation();
  const [toast, setToast] = useState('');
  const { activeDepts } = useDeptEvents();`
);
addPatch('src/components/ChatPanel.tsx',
  `      await cancelProcessing();
      showToast('已叫停诸司');
    } catch {
      showToast('叫停失败');
    }`,
  `      await cancelProcessing();
      showToast(t('chat.stopAllDepts'));
    } catch {
      showToast(t('common.error'));
    }`
);
addPatch('src/components/ChatPanel.tsx',
  `            叫停诸司`,
  `            {t('chat.stopAllDepts')}`
);
addPatch('src/components/ChatPanel.tsx',
  `          placeholder={isProcessing ? '诸司处理中…' : '拟旨…'}`,
  `          placeholder={isProcessing ? t('chat.processing') : t('chat.inputPlaceholder')}`
);
addPatch('src/components/ChatPanel.tsx',
  `            叫停讨论`,
  `            {t('common.cancel')}`
);
addPatch('src/components/ChatPanel.tsx',
  `            将此事转为正式敕命`,
  `            {t('chat.send')}`
);
addPatch('src/components/ChatPanel.tsx',
  `      <ChatInput onSend={onDiscuss} disabled={discussing} placeholder="廷议…" />`,
  `      <ChatInput onSend={onDiscuss} disabled={discussing} placeholder={t('chat.discussing')} />`
);
addPatch('src/components/ChatPanel.tsx',
  `            <span className="text-ui text-ink-600 font-display">诸司处理中…</span>`,
  `            <span className="text-ui text-ink-600 font-display">{t('chat.processing')}</span>`
);
addPatch('src/components/ChatPanel.tsx',
  `      <div className="font-display text-caption text-ink-600 font-semibold mb-1">工部计划</div>`,
  `      <div className="font-display text-caption text-ink-600 font-semibold mb-1">{t('chat.gongbuPlan')}</div>`
);

// ── ChatBubble ──
addPatch('src/components/ChatBubble.tsx',
  `import { useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeHighlight from 'rehype-highlight';
import DeptGlyph from './DeptGlyph';
import type { ChatMessage, ChatOption } from '../types';
import { getDeptMeta } from '../constants';`,
  `import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeHighlight from 'rehype-highlight';
import DeptGlyph from './DeptGlyph';
import type { ChatMessage, ChatOption } from '../types';
import { getDeptMeta } from '../constants';`
);
addPatch('src/components/ChatBubble.tsx',
  `  const isEmperor = msg.role === '皇帝';
  const isFailed = msg.status === 'failed';
  const meta = getDeptMeta(msg.role);`,
  `  const { t } = useTranslation();
  const isEmperor = msg.role === '皇帝';
  const isFailed = msg.status === 'failed';
  const meta = getDeptMeta(msg.role);`
);
addPatch('src/components/ChatBubble.tsx',
  `                <span className="inline-flex w-4 h-4 items-center justify-center rounded-sm border border-vermillion/50 text-vermillion text-[10px] font-display leading-none">
                  御
                </span>
                圣旨`,
  `                <span className="inline-flex w-4 h-4 items-center justify-center rounded-sm border border-vermillion/50 text-vermillion text-[10px] font-display leading-none">
                  {t('chat.emperor')}
                </span>
                {t('chat.imperialEdict')}`
);
addPatch('src/components/ChatBubble.tsx',
  `                  <span className="text-caption text-vermillion">发送失败</span>
                  <button
                    onClick={() => onRetry(msg.content, msg.timestamp)}
                    className="text-caption font-semibold px-2 py-0.5 rounded bg-vermillion text-white hover:bg-vermillion-dark"
                  >
                    重试`,
  `                  <span className="text-caption text-vermillion">{t('chat.sendFailed')}</span>
                  <button
                    onClick={() => onRetry(msg.content, msg.timestamp)}
                    className="text-caption font-semibold px-2 py-0.5 rounded bg-vermillion text-white hover:bg-vermillion-dark"
                  >
                    {t('chat.retry')}`
);
addPatch('src/components/ChatBubble.tsx',
  `                <span className="text-caption text-ink-400">回奏</span>`,
  `                <span className="text-caption text-ink-400">{t('chat.reply')}</span>`
);
addPatch('src/components/ChatBubble.tsx',
  `          placeholder="在此补充御批..."`,
  `          placeholder={t('chat.addNote')}`
);
addPatch('src/components/ChatBubble.tsx',
  `            遵旨`,
  `            {t('chat.confirm')}`
);
addPatch('src/components/ChatBubble.tsx',
  `            作罢`,
  `            {t('chat.dismissed')}`
);

// ── ChatInput ──
addPatch('src/components/ChatInput.tsx',
  `import { useState, useRef, useEffect, useCallback, useImperativeHandle, forwardRef } from 'react';
import { setDotenvKey } from '../api';`,
  `import { useState, useRef, useEffect, useCallback, useImperativeHandle, forwardRef } from 'react';
import { useTranslation } from 'react-i18next';
import { setDotenvKey } from '../api';`
);
addPatch('src/components/ChatInput.tsx',
  `export default forwardRef<ChatInputHandle, Props>(function ChatInput({ onSend, disabled, placeholder }, ref) {
  const [text, setText] = useState('');
  const [toast, setToast] = useState('');`,
  `export default forwardRef<ChatInputHandle, Props>(function ChatInput({ onSend, disabled, placeholder }, ref) {
  const { t } = useTranslation();
  const [text, setText] = useState('');
  const [toast, setToast] = useState('');`
);
addPatch('src/components/ChatInput.tsx',
  `  '/level-1': { level: '1', label: '全自动' },
  '/level-2': { level: '2', label: '关键节点确认' },
  '/level-3': { level: '3', label: '逐步审核' },
  '/auto': { level: '1', label: '全自动' },
  '/step': { level: '2', label: '关键节点确认' },
  '/detail': { level: '3', label: '逐步审核' },`,
  `  '/level-1': { level: '1', label: 'chat.commandLevelAuto' },
  '/level-2': { level: '2', label: 'chat.commandLevelConfirm' },
  '/level-3': { level: '3', label: 'chat.commandLevelReview' },
  '/auto': { level: '1', label: 'chat.commandLevelAuto' },
  '/step': { level: '2', label: 'chat.commandLevelConfirm' },
  '/detail': { level: '3', label: 'chat.commandLevelReview' },`
);
addPatch('src/components/ChatInput.tsx',
  `      } catch {
        showToast('切换失败');
      }`,
  `      } catch {
        showToast(t('common.error'));
      }`
);
addPatch('src/components/ChatInput.tsx',
  `          placeholder={placeholder || '/level-1 /level-2 /level-3 切换参与度'}`,
  `          placeholder={placeholder || t('chat.inputPlaceholder')}`
);
addPatch('src/components/ChatInput.tsx',
  `          下诏`,
  `          {t('chat.send')}`
);

// ── AgentStreamPanel ──
addPatch('src/components/AgentStreamPanel.tsx',
  `import { useState, useEffect, useRef } from 'react';
import { Tabs } from './ui/Tabs';
import CommandBar from './CommandBar';
import DeptCardRail from './DeptCardRail';
import DeptInspector from './DeptInspector';
import ChatPanel from './ChatPanel';
import AgentIdleState from './AgentIdleState';
import { getDeptMeta } from '../constants';
import { useDeptEvents } from '../hooks/useDeptEvents';
import type { Project, ChatMessage, PlanInfo } from '../types';
import type { Tab } from '../hooks/useChat';
import type { PhaseRuntime } from '../types';
import type { ChatInputHandle } from './ChatInput';`,
  `import { useState, useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { Tabs } from './ui/Tabs';
import CommandBar from './CommandBar';
import DeptCardRail from './DeptCardRail';
import DeptInspector from './DeptInspector';
import ChatPanel from './ChatPanel';
import AgentIdleState from './AgentIdleState';
import { getDeptMeta } from '../constants';
import { useDeptEvents } from '../hooks/useDeptEvents';
import type { Project, ChatMessage, PlanInfo } from '../types';
import type { Tab } from '../hooks/useChat';
import type { PhaseRuntime } from '../types';
import type { ChatInputHandle } from './ChatInput';`
);
addPatch('src/components/AgentStreamPanel.tsx',
  `  const { latestLogs, logEntries } = useDeptEvents();`,
  `  const { t } = useTranslation();
  const { latestLogs, logEntries } = useDeptEvents();`
);
addPatch('src/components/AgentStreamPanel.tsx',
  `                <span className="font-display text-ui font-semibold text-ink-800">拟旨殿</span>
                <Tabs
                  tabs={[
                    { key: 'decision', label: '决策' },
                    { key: 'discuss', label: '廷议' },
                  ]}`,
  `                <span className="font-display text-ui font-semibold text-ink-800">{t('inspector.backToDuty')}</span>
                <Tabs
                  tabs={[
                    { key: 'decision', label: t('inspector.decision') },
                    { key: 'discuss', label: t('inspector.discussion') },
                  ]}`
);
addPatch('src/components/AgentStreamPanel.tsx',
  `          请先开卷`,
  `          {t('inspector.pleaseOpen')}`
);

// ── DeptInspector ──
addPatch('src/components/DeptInspector.tsx',
  `import { useRef, useEffect, useState } from 'react';
import { getDeptMeta } from '../constants';
import DeptActivityCard from './DeptActivityCard';
import RouteContextBar from './RouteContextBar';
import { SealLogo } from './SealLogo';
import { useDeptEvents } from '../hooks/useDeptEvents';
import type { DeptLogEntry, DeptStepEntry, PlanInfo } from '../types';`,
  `import { useRef, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { getDeptMeta } from '../constants';
import DeptActivityCard from './DeptActivityCard';
import RouteContextBar from './RouteContextBar';
import { SealLogo } from './SealLogo';
import { useDeptEvents } from '../hooks/useDeptEvents';
import type { DeptLogEntry, DeptStepEntry, PlanInfo } from '../types';`
);
addPatch('src/components/DeptInspector.tsx',
  `  const color = meta?.color || '#8B7355';
  const latestEntry = entries.length > 0 ? entries[entries.length - 1] : null;`,
  `  const { t } = useTranslation();
  const color = meta?.color || '#8B7355';
  const latestEntry = entries.length > 0 ? entries[entries.length - 1] : null;`
);
addPatch('src/components/DeptInspector.tsx',
  `          返拟旨殿`,
  `          {t('inspector.backToDuty')}`
);
addPatch('src/components/DeptInspector.tsx',
  `            {hasError ? '出错' : active ? '执行中' : '空闲'}`,
  `            {hasError ? t('inspector.error') : active ? t('inspector.executing') : t('inspector.idle')}`
);
addPatch('src/components/DeptInspector.tsx',
  `        <div className="text-ui text-ink-400">该司暂无奏报</div>`,
  `        <div className="text-ui text-ink-400">{t('inspector.noReports')}</div>`
);
addPatch('src/components/DeptInspector.tsx',
  `            滚动到底部`,
  `            {t('common.refresh')}`
);
addPatch('src/components/DeptInspector.tsx',
  `      <div className="font-display text-caption text-ink-600 font-semibold mb-1">工部批次</div>`,
  `      <div className="font-display text-caption text-ink-600 font-semibold mb-1">{t('inspector.gongbuBatch')}</div>`
);
addPatch('src/components/DeptInspector.tsx',
  `            <span className="text-jade font-semibold">思考过程</span>`,
  `            <span className="text-jade font-semibold">{t('inspector.thinking')}</span>`
);
addPatch('src/components/DeptInspector.tsx',
  `            返拟旨殿
          </button>
          <span className="text-ui font-semibold text-ink-700">诸司动态</span>
          <span className="text-caption text-ink-400">{entries.length} 条`,
  `            {t('inspector.backToDuty')}
          </button>
          <span className="text-ui font-semibold text-ink-700">{t('inspector.deptActivity')}</span>
          <span className="text-caption text-ink-400">{entries.length} {t('common.noRecords')}`.replace('{t(\'common.noRecords\')}', '') + `</span>`
);
console.log('Patches defined');
