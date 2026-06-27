import { useState, useEffect, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import {
  sendMessage,
  discussStream,
  cancelDiscuss as cancelDiscussApi,
  getChatHistory,
} from '../api';
import { formatError } from '../utils/error';
import { initialCabinetMessage, mergeMessages } from '../utils/chat';
import type { ChatDeltaEvent, ChatMessage, PlanInfo } from '../types';

export type Tab = 'decision' | 'discuss';

export function useChat(initialMessages: ChatMessage[]) {
  const { t } = useTranslation();
  const [messages, setMessages] = useState<ChatMessage[]>(initialMessages);
  const [discussMsgs, setDiscussMsgs] = useState<ChatMessage[]>([
    initialCabinetMessage(t('chat.welcomeDiscuss')),
  ]);
  const [discussing, setDiscussing] = useState(false);
  const [tab, setTab] = useState<Tab>('decision');
  const [planInfo, setPlanInfo] = useState<PlanInfo | null>(null);
  const [error, setError] = useState('');
  const chatEndRef = useRef<HTMLDivElement>(null);
  const discussCancelRef = useRef(false);
  const streamingIdRef = useRef<string | null>(null);

  // Scroll to end on new messages
  useEffect(() => {
    chatEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, discussMsgs, tab]);

  // Restore chat history
  useEffect(() => {
    getChatHistory()
      .then((hist) => {
        if (hist.length > 0) setMessages((prev) => mergeMessages(prev, hist));
      })
      .catch((e) => setError(t('chat.loadHistoryFailed', { error: formatError(e) })));
  }, []);

  // Listen for real-time chat messages
  useEffect(() => {
    const unlisten = listen<ChatMessage>('chat-message', (event) =>
      setMessages((prev) => [...prev, event.payload])
    );
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  // Listen for discuss stream deltas and completion
  useEffect(() => {
    const unlistenDelta = listen<ChatDeltaEvent>('chat-delta', (event) => {
      const { message_id, delta } = event.payload;
      if (discussCancelRef.current) return;
      setDiscussMsgs((prev) =>
        prev.map((m) =>
          m.id === message_id ? { ...m, content: m.content + delta, streaming: true } : m
        )
      );
    });

    const unlistenComplete = listen<ChatMessage>('chat-complete', (event) => {
      const msg = { ...event.payload, streaming: false };
      if (discussCancelRef.current && streamingIdRef.current === msg.id) {
        return;
      }
      setDiscussMsgs((prev) => {
        const idx = prev.findIndex((m) => m.id === msg.id);
        if (idx >= 0) {
          const next = [...prev];
          next[idx] = msg;
          return next;
        }
        return [...prev, msg];
      });
      if (streamingIdRef.current === msg.id) {
        streamingIdRef.current = null;
        setDiscussing(false);
        discussCancelRef.current = false;
      }
    });

    return () => {
      unlistenDelta.then((f) => f());
      unlistenComplete.then((f) => f());
    };
  }, []);

  // Listen for plan updates
  useEffect(() => {
    const unlisten = listen<PlanInfo>('plan-update', (event) =>
      setPlanInfo(event.payload.complete ? null : event.payload)
    );
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const handleSend = async (text: string) => {
    const ts = new Date().toISOString();
    setError('');
    const msg: ChatMessage = {
      id: crypto.randomUUID(),
      role: '皇帝',
      content: text,
      options: [],
      documents: [],
      timestamp: ts,
    };
    setMessages((prev) => [...prev, msg]);
    try {
      await sendMessage(text);
    } catch (e) {
      setError(formatError(e));
      setMessages((prev) =>
        prev.map((m) => (m.timestamp === ts && m.role === '皇帝' ? { ...m, status: 'failed' } : m))
      );
    }
  };

  const retrySend = async (text: string, originalTs: string) => {
    setMessages((prev) => prev.filter((m) => m.timestamp !== originalTs || m.role !== '皇帝'));
    await handleSend(text);
  };

  const handleDiscuss = async (text: string) => {
    discussCancelRef.current = false;
    setDiscussing(true);

    const streamId = crypto.randomUUID();
    streamingIdRef.current = streamId;
    const ts = new Date().toISOString();

    setDiscussMsgs((prev) => [
      ...prev,
      {
        id: crypto.randomUUID(),
        role: '皇帝',
        content: text,
        options: [],
        documents: [],
        timestamp: ts,
      },
      {
        id: streamId,
        role: '内阁',
        content: '',
        options: [],
        documents: [],
        timestamp: ts,
        streaming: true,
      },
    ]);

    try {
      await discussStream(text, streamId);
      if (discussCancelRef.current) return;
    } catch (e) {
      if (!discussCancelRef.current) {
        setDiscussMsgs((prev) => {
          const withoutPlaceholder = prev.filter((m) => m.id !== streamId);
          return [
            ...withoutPlaceholder,
            initialCabinetMessage(t('chat.discussError', { error: formatError(e) })),
          ];
        });
      }
      streamingIdRef.current = null;
      setDiscussing(false);
      discussCancelRef.current = false;
    }
  };

  const cancelDiscuss = useCallback(() => {
    if (discussing) {
      discussCancelRef.current = true;
      const cancelledId = streamingIdRef.current;
      streamingIdRef.current = null;
      setDiscussing(false);
      cancelDiscussApi().catch(() => {});
      setDiscussMsgs((prev) => {
        const withoutPartial =
          cancelledId != null ? prev.filter((m) => m.id !== cancelledId) : prev;
        return [...withoutPartial, initialCabinetMessage(t('chat.discussCancelled'))];
      });
    }
  }, [discussing, t]);

  const resetDiscuss = () => setDiscussMsgs([initialCabinetMessage(t('chat.welcomeDiscuss'))]);

  return {
    messages,
    discussMsgs,
    discussing,
    tab,
    planInfo,
    error,
    setError,
    setTab,
    setMessages,
    handleSend,
    retrySend,
    handleDiscuss,
    cancelDiscuss,
    resetDiscuss,
    chatEndRef,
  };
}
