import { SettingsSection, SettingsAction, SettingsHint } from './SettingsPrimitives';

interface SoulSettingsTabProps {
  setSavedMsg: (msg: string) => void;
}

export default function SoulSettingsTab({ setSavedMsg }: SoulSettingsTabProps) {
  return (
    <SettingsSection
      title="Soul 管理"
      description="内阁跨会话积累的经验、教训与偏好，存储于 .shuji/soul.md"
    >
      <div className="flex gap-2 flex-wrap">
        <SettingsAction
          onClick={async () => {
            try {
              const { getSoulContent } = await import('../../api');
              const content = await getSoulContent();
              if (!content) {
                setSavedMsg('soul 为空或不存在');
                setTimeout(() => setSavedMsg(''), 2000);
                return;
              }
              await navigator.clipboard.writeText(content);
              setSavedMsg('soul 已复制到剪贴板');
              setTimeout(() => setSavedMsg(''), 2000);
            } catch (e) {
              setSavedMsg(String(e));
            }
          }}
        >
          导出 soul（复制）
        </SettingsAction>
        <SettingsAction
          variant="danger"
          onClick={async () => {
            try {
              const { clearSoul } = await import('../../api');
              await clearSoul();
              setSavedMsg('soul 已重置为默认');
              setTimeout(() => setSavedMsg(''), 2000);
            } catch (e) {
              setSavedMsg(String(e));
            }
          }}
        >
          清空 soul
        </SettingsAction>
      </div>
      <SettingsHint>
        soul 超过 8KB 时将自动压缩。单条经验/教训/偏好不超过 500 字符。
      </SettingsHint>
    </SettingsSection>
  );
}
