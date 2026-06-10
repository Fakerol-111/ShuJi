/**
 * Document tab management for ProjectDashboard.
 * Handles open/close/switch of document tabs with initial view hints.
 */
import { useState, useCallback } from 'react';
import type { TabInfo } from '../components/TabBar';

function tabLabelFromPath(path: string): string {
  const name = path.split('/').pop() || path;
  return name.replace(/\.md$/, '');
}

export function useDocumentTabs() {
  const [tabs, setTabs] = useState<TabInfo[]>([]);
  const [activeIndex, setActiveIndex] = useState(-1);

  const openTab = useCallback((path: string, initialView?: TabInfo['initialView']) => {
    setTabs((prev) => {
      const idx = prev.findIndex((t) => t.path === path);
      if (idx >= 0) {
        setActiveIndex(idx);
        if (initialView) {
          const updated = [...prev];
          updated[idx] = { ...updated[idx], initialView };
          return updated;
        }
        return prev;
      }
      setActiveIndex(prev.length);
      return [...prev, { path, label: tabLabelFromPath(path), initialView }];
    });
  }, []);

  const closeTab = useCallback((index: number) => {
    setTabs((prev) => {
      if (index < 0 || index >= prev.length) return prev;
      const next = prev.filter((_, i) => i !== index);
      setActiveIndex((current) => {
        if (current === index) {
          return index > 0 ? index - 1 : next.length > 0 ? 0 : -1;
        }
        if (current > index) return current - 1;
        return current;
      });
      return next;
    });
  }, []);

  const handleDocSelect = useCallback((path: string) => openTab(path), [openTab]);

  const activeDoc = activeIndex >= 0 && activeIndex < tabs.length ? tabs[activeIndex] : null;
  const hasTabs = tabs.length > 0 && activeDoc !== null;

  return {
    tabs,
    activeIndex,
    activeDoc,
    hasTabs,
    openTab,
    closeTab,
    handleDocSelect,
    setActiveIndex,
  };
}
