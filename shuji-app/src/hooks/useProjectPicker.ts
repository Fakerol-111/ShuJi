import { useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { getRecentDirs } from '../api';
import { formatError } from '../utils/error';

export function useProjectPicker(
  loadProjectIntoState: (path: string) => Promise<void>,
  setRecentDirs: (dirs: string[]) => void
) {
  const [showPicker, setShowPicker] = useState(false);
  const [pickerPath, setPickerPath] = useState('');
  const [pickerLoading, setPickerLoading] = useState(false);
  const [pickerError, setPickerError] = useState('');

  const openPicker = () => {
    setPickerPath('');
    setPickerError('');
    getRecentDirs()
      .then(setRecentDirs)
      .catch((e) => {
        console.error('获取最近目录失败', e);
        setPickerError(formatError(e));
      });
    setShowPicker(true);
  };

  const onBrowse = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: '选择工作目录',
      });
      if (selected) setPickerPath(selected);
    } catch (e) {
      setPickerError(formatError(e));
    }
  };

  const onLoad = async (dir?: string) => {
    const path = dir || pickerPath.trim();
    if (!path) {
      setPickerError('请选择工作目录');
      return;
    }
    setPickerLoading(true);
    setPickerError('');
    try {
      await loadProjectIntoState(path);
      sessionStorage.removeItem('shuji_chat');
      setShowPicker(false);
    } catch (e) {
      setPickerError(formatError(e));
    } finally {
      setPickerLoading(false);
    }
  };

  return {
    showPicker,
    setShowPicker,
    pickerPath,
    setPickerPath,
    pickerError,
    pickerLoading,
    openPicker,
    onBrowse,
    onLoad,
  };
}
