import { useState, useEffect } from 'react';
import {
  CODE_THEMES,
  FONT_SIZE_TIERS,
  getCodeTheme,
  setCodeTheme as persistCodeTheme,
  getFontSize,
  setFontSize as persistFontSize,
} from '../../constants';
import { SettingsSection, SettingsChip, SettingsHint } from './SettingsPrimitives';

export default function AppearanceTab() {
  const [codeTheme, setCodeThemeLocal] = useState(getCodeTheme);
  const [fontSize, setFontSizeLocal] = useState(getFontSize);

  useEffect(() => {
    document.documentElement.dataset.fontSize = fontSize;
  }, [fontSize]);

  useEffect(() => {
    document.documentElement.dataset.codeTheme = codeTheme;
  }, [codeTheme]);

  return (
    <div className="space-y-6">
      <SettingsSection title="字体大小" description="修改即时生效，无需保存。">
        <div className="flex gap-2 flex-wrap">
          {Object.entries(FONT_SIZE_TIERS).map(([key, tier]) => (
            <SettingsChip
              key={key}
              selected={fontSize === key}
              onClick={() => {
                setFontSizeLocal(key);
                persistFontSize(key);
              }}
              title={tier.description}
            >
              {tier.label}
            </SettingsChip>
          ))}
        </div>
        <SettingsHint>
          {FONT_SIZE_TIERS[fontSize as keyof typeof FONT_SIZE_TIERS]?.description}
        </SettingsHint>
      </SettingsSection>

      <SettingsSection title="代码主题" description="修改即时生效，无需保存。" divider>
        <div className="flex gap-2 flex-wrap">
          {Object.entries(CODE_THEMES).map(([key, theme]) => (
            <SettingsChip
              key={key}
              selected={codeTheme === key}
              onClick={() => {
                setCodeThemeLocal(key);
                persistCodeTheme(key);
              }}
            >
              {theme.label}
            </SettingsChip>
          ))}
        </div>
      </SettingsSection>

      <SettingsSection title="关于枢机" divider>
        <div className="text-sm text-ink-700 space-y-2 leading-relaxed">
          <p>
            <span className="font-medium text-ink-900">版本</span> 0.1.0 — 预览版
          </p>
          <p>
            基于三省六部制的自动化软件开发系统。每个部门是一个 LLM
            agent，通过角色分工和文档化通信，模拟从需求分析到编码测试的完整软件工程流程。
          </p>
        </div>
      </SettingsSection>
    </div>
  );
}
