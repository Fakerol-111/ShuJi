import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import rehypeHighlight from 'rehype-highlight';
import DeptGlyph from './DeptGlyph';
import type { ChatMessage, ChatOption } from '../types';
import { getDeptMeta } from '../constants';

export default function ChatBubble({
  msg,
  onOption,
  onRetry,
}: {
  msg: ChatMessage;
  onOption: (key: string, supplement?: string) => void;
  onRetry?: (text: string, ts: string) => void;
}) {
  const { t } = useTranslation();
  const isEmperor = msg.role === '皇帝';
  const isFailed = msg.status === 'failed';
  const meta = getDeptMeta(msg.role);

  return (
    <div className={`flex ${isEmperor ? 'justify-end' : 'justify-start'}`}>
      <div className={`${isEmperor ? 'max-w-[70%]' : 'max-w-[85%]'}`}>
        {isEmperor ? (
          <>
            <div className="text-right text-caption text-ink-500 mb-1">
              <span className="inline-flex items-center gap-1">
                <span className="inline-flex w-4 h-4 items-center justify-center rounded-sm border border-vermillion/50 text-vermillion text-[10px] font-display leading-none">
                  {t('chat.emperor')}
                </span>
                {t('chat.imperialEdict')}
              </span>
            </div>
            <div
              className={`rounded-xl rounded-tr-sm px-4 py-2.5 text-body leading-relaxed border border-gold/20 shadow-sm ${
                isFailed
                  ? 'bg-vermillion/10 border-vermillion/30 text-ink-800'
                  : 'bg-ink-900 text-ink-50'
              }`}
            >
              <p className="whitespace-pre-wrap break-words overflow-hidden">{msg.content}</p>
              {isFailed && onRetry && (
                <div className="flex items-center gap-2 mt-2 pt-2 border-t border-vermillion/20">
                  <span className="text-caption text-vermillion">{t('chat.sendFailed')}</span>
                  <button
                    onClick={() => onRetry(msg.content, msg.timestamp)}
                    className="text-caption font-semibold px-2 py-0.5 rounded bg-vermillion text-white hover:bg-vermillion-dark"
                  >
                    {t('chat.retry')}
                  </button>
                </div>
              )}
            </div>
          </>
        ) : (
          <>
            {meta && (
              <div className="flex items-center gap-2 mb-2 pb-1 border-b border-fold/60">
                <DeptGlyph deptKey={meta.key} size={14} stroke={meta.color} />
                <span className="font-display text-ui font-semibold" style={{ color: meta.color }}>
                  {msg.role}
                </span>
                <span className="text-caption text-ink-400">{t('chat.reply')}</span>
              </div>
            )}
            <div
              className="bg-surface-elevated border border-fold rounded-xl rounded-tl-sm px-4 py-2.5 text-body leading-relaxed"
              style={meta?.color ? { borderLeft: `3px solid ${meta.color}` } : undefined}
            >
              <div className="prose prose-shuji max-w-none break-words">
                <ReactMarkdown
                  remarkPlugins={[remarkGfm]}
                  rehypePlugins={[rehypeHighlight]}
                  components={{
                    a: ({ href, children }) => (
                      <a
                        href={href}
                        target="_blank"
                        rel="noopener noreferrer"
                        onClick={(e) => {
                          e.preventDefault();
                          if (href) window.open(href, '_blank');
                        }}
                      >
                        {children}
                      </a>
                    ),
                  }}
                >
                  {msg.content}
                </ReactMarkdown>
              </div>
            </div>
          </>
        )}

        {msg.options.length > 0 && <OptionGroup options={msg.options} onOption={onOption} />}
      </div>
    </div>
  );
}

function OptionGroup({
  options,
  onOption,
}: {
  options: ChatOption[];
  onOption: (key: string, supplement?: string) => void;
}) {
  const { t } = useTranslation();
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [supplement, setSupplement] = useState('');

  if (selectedKey) {
    const opt = options.find((o) => o.key === selectedKey);
    if (!opt) return null;
    return (
      <div className="mt-2 bg-ink-100 border border-fold rounded-lg p-3">
        <p className="text-ui font-bold text-ink-800 mb-1">
          {opt.key}. {opt.label}
        </p>
        <p className="text-caption text-ink-600 mb-2">{opt.description}</p>
        <textarea
          className="w-full border border-fold rounded px-3 py-2 text-body resize-none bg-surface-elevated text-ink-900 focus:outline-none focus:border-vermillion"
          rows={3}
          placeholder={t('chat.addNote')}
          value={supplement}
          onChange={(e) => setSupplement(e.target.value)}
        />
        <div className="flex gap-2 mt-2">
          <button
            onClick={() => onOption(selectedKey, supplement)}
            className="bg-vermillion text-white text-ui font-bold px-4 py-1.5 rounded-lg hover:bg-vermillion-dark transition-colors"
          >
            {t('common.confirm')}
          </button>
          <button
            onClick={() => {
              setSelectedKey(null);
              setSupplement('');
            }}
            className="text-ui text-ink-500 px-3 py-1.5 hover:text-ink-700"
          >
            {t('chat.dismissed')}
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-wrap gap-1.5 mt-2">
      {options.map((opt) => (
        <button
          key={opt.key}
          onClick={() => {
            if (opt.label.includes('补充') || opt.key === 'C') {
              setSelectedKey(opt.key);
            } else {
              onOption(opt.key);
            }
          }}
          className={`text-ui font-bold px-3 py-1.5 rounded-lg transition-colors border ${
            opt.key === 'A'
              ? 'border-jade bg-jade-light text-jade hover:bg-jade'
              : opt.key === 'B'
                ? 'border-vermillion bg-vermillion-light text-vermillion hover:bg-vermillion'
                : opt.key === 'C'
                  ? 'border-ink-300 bg-surface-elevated text-ink-800 hover:border-vermillion/40'
                  : 'border-fold bg-surface-elevated text-ink-800 hover:border-vermillion/40'
          }`}
          title={opt.description}
        >
          {opt.key}. {opt.label}
        </button>
      ))}
    </div>
  );
}
