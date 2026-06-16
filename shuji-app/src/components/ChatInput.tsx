import { useState, useRef, useEffect, useCallback, useImperativeHandle, forwardRef } from 'react';
import { useTranslation } from 'react-i18next';
import { setDotenvKey } from '../api';

export interface ChatInputHandle {
  setText: (text: string) => void;
}

interface Props {
  onSend: (msg: string) => void;
  disabled?: boolean;
  placeholder?: string;
}

const LINE_HEIGHT = 20;
const MAX_LINES = 4;

const SLASH_COMMANDS: Record<string, { level: string; label: string }> = {
  '/level-1': { level: '1', label: 'chat.commandLevelAuto' },
  '/level-2': { level: '2', label: 'chat.commandLevelConfirm' },
  '/level-3': { level: '3', label: 'chat.commandLevelReview' },
  '/auto': { level: '1', label: 'chat.commandLevelAuto' },
  '/step': { level: '2', label: 'chat.commandLevelConfirm' },
  '/detail': { level: '3', label: 'chat.commandLevelReview' },
};

export default forwardRef<ChatInputHandle, Props>(function ChatInput({ onSend, disabled, placeholder }, ref) {
  const { t } = useTranslation();
  const [text, setText] = useState('');
  const [toast, setToast] = useState('');
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useImperativeHandle(ref, () => ({ setText }));

  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  const adjustHeight = useCallback(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = 'auto';
    const maxH = LINE_HEIGHT * MAX_LINES;
    el.style.height = `${Math.min(el.scrollHeight, maxH)}px`;
  }, []);

  useEffect(() => {
    adjustHeight();
  }, [text, adjustHeight]);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(''), 2000);
  };

  const handleSend = async () => {
    const trimmed = text.trim();
    if (!trimmed || disabled) return;

    // Check for slash commands
    const words = trimmed.split(/\s+/);
    const cmd = words[0];
    if (SLASH_COMMANDS[cmd]) {
      const { level, label } = SLASH_COMMANDS[cmd];
      try {
        await setDotenvKey('PARTICIPATION_LEVEL', level);
        showToast(`${t(label)} (${t('chat.commandLevel')} ${level})`);
      } catch {
        showToast(t('common.error'));
      }
      // If there's more text after the command, send it
      const rest = words.slice(1).join(' ');
      if (rest) {
        onSend(rest);
      }
      setText('');
      return;
    }

    onSend(trimmed);
    setText('');
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div className="relative">
      {toast && (
        <div className="absolute -top-9 left-4 right-4 text-center text-xs text-vermillion bg-red-50 rounded px-3 py-1 animate-pulse">
          {toast}
        </div>
      )}
      <div className="flex gap-2 edict-input-wrap px-4 py-3 items-end">
        <textarea
          ref={textareaRef}
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={placeholder || t('chat.inputPlaceholder')}
          disabled={disabled}
          rows={1}
          className="flex-1 px-3 py-2 border border-fold bg-surface-parchment rounded-lg text-body text-ink-900 placeholder:text-ink-400 focus:outline-none focus:border-vermillion focus:ring-1 focus:ring-vermillion/30 disabled:opacity-50 resize-none leading-5"
        />
        <button
          onClick={handleSend}
          disabled={disabled || !text.trim()}
          className="px-5 py-2 bg-ink-900 text-ink-50 rounded-lg hover:bg-ink-800 disabled:opacity-40 text-ui font-medium transition-colors shrink-0"
        >
          {t('chat.send')}
        </button>
      </div>
    </div>
  );
});
