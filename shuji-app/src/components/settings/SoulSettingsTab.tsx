import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { SettingsSection, SettingsAction, SettingsHint } from './SettingsPrimitives';

interface SoulSettingsTabProps {
  setSavedMsg: (msg: string) => void;
}

export default function SoulSettingsTab({ setSavedMsg }: SoulSettingsTabProps) {
  const { t } = useTranslation();
  const [roles, setRoles] = useState<string[]>([]);
  const [selectedRole, setSelectedRole] = useState('Neige');
  const [scope, setScope] = useState<'project' | 'global'>('project');
  const [preview, setPreview] = useState('');
  const [globalEnabled, setGlobalEnabled] = useState(false);
  const [candidates, setCandidates] = useState<import('../../api').LearningEntry[]>([]);

  const loadRoles = async () => {
    const { listSoulRoles } = await import('../../api');
    const list = await listSoulRoles();
    setRoles(list.length > 0 ? list : ['Neige']);
  };

  const loadPreview = async (role: string, nextScope: 'project' | 'global') => {
    const { getSoulContent } = await import('../../api');
    const content = await getSoulContent(role, nextScope);
    setPreview(content || t('common.noData'));
  };

  const loadCandidates = async () => {
    const { listGlobalLearningCandidates } = await import('../../api');
    const list = await listGlobalLearningCandidates();
    setCandidates(list);
  };

  const loadConfig = async () => {
    const { getLearningConfig } = await import('../../api');
    const cfg = await getLearningConfig();
    setGlobalEnabled(cfg.global_enabled);
  };

  useEffect(() => {
    void loadRoles();
    void loadConfig();
    void loadCandidates();
  }, []);

  useEffect(() => {
    void loadPreview(selectedRole, scope);
  }, [selectedRole, scope]);

  return (
    <SettingsSection
      title={t('settings.soulManagement')}
      description={t('settings.soulDescription')}
    >
      <div className="flex flex-col gap-3">
        <div className="flex gap-2 flex-wrap items-center">
          <label className="text-sm text-muted-foreground">{t('settings.soulRoleLabel')}</label>
          <select
            className="border rounded px-2 py-1 text-sm bg-background"
            value={selectedRole}
            onChange={(e) => setSelectedRole(e.target.value)}
          >
            {roles.map((role) => (
              <option key={role} value={role}>
                {role}
              </option>
            ))}
          </select>
          <select
            className="border rounded px-2 py-1 text-sm bg-background"
            value={scope}
            onChange={(e) => setScope(e.target.value as 'project' | 'global')}
          >
            <option value="project">{t('settings.soulScopeProject')}</option>
            <option value="global">{t('settings.soulScopeGlobal')}</option>
          </select>
          <label className="flex items-center gap-1 text-sm">
            <input
              type="checkbox"
              checked={globalEnabled}
              onChange={async (e) => {
                const { setLearningGlobalEnabled } = await import('../../api');
                await setLearningGlobalEnabled(e.target.checked);
                setGlobalEnabled(e.target.checked);
                setSavedMsg(t('common.saved'));
                setTimeout(() => setSavedMsg(''), 2000);
              }}
            />
            {t('settings.soulGlobalLearning')}
          </label>
        </div>

        <pre className="text-xs max-h-40 overflow-auto border rounded p-2 whitespace-pre-wrap">
          {preview}
        </pre>

        <div className="flex gap-2 flex-wrap">
          <SettingsAction
            onClick={async () => {
              try {
                const { getSoulContent } = await import('../../api');
                const content = await getSoulContent(selectedRole, scope);
                if (!content) {
                  setSavedMsg(t('common.noData'));
                  setTimeout(() => setSavedMsg(''), 2000);
                  return;
                }
                await navigator.clipboard.writeText(content);
                setSavedMsg(t('common.saved'));
                setTimeout(() => setSavedMsg(''), 2000);
              } catch (e) {
                setSavedMsg(String(e));
              }
            }}
          >
            {t('common.export')}
          </SettingsAction>
          <SettingsAction
            variant="danger"
            onClick={async () => {
              try {
                const { clearSoul } = await import('../../api');
                await clearSoul(selectedRole, scope);
                await loadPreview(selectedRole, scope);
                setSavedMsg(t('common.saved'));
                setTimeout(() => setSavedMsg(''), 2000);
              } catch (e) {
                setSavedMsg(String(e));
              }
            }}
          >
            {t('common.delete')}
          </SettingsAction>
        </div>

        {candidates.length > 0 && (
          <div className="border rounded p-2 space-y-2">
            <div className="text-sm font-medium">{t('settings.soulPendingCandidates')}</div>
            {candidates.map((c) => (
              <div key={c.id} className="text-xs border-b pb-2 last:border-0">
                <div>
                  [{c.role}] {c.content}
                </div>
                {c.evidence.length > 0 && (
                  <div className="text-muted-foreground mt-1">
                    evidence: {c.evidence.join(', ')}
                  </div>
                )}
                <div className="flex gap-2 mt-1">
                  <SettingsAction
                    onClick={async () => {
                      const { approveGlobalLearning } = await import('../../api');
                      await approveGlobalLearning(c.id);
                      await loadCandidates();
                      setSavedMsg(t('common.saved'));
                      setTimeout(() => setSavedMsg(''), 2000);
                    }}
                  >
                    {t('settings.soulApprove')}
                  </SettingsAction>
                  <SettingsAction
                    variant="danger"
                    onClick={async () => {
                      const { rejectGlobalLearning } = await import('../../api');
                      await rejectGlobalLearning(c.id);
                      await loadCandidates();
                      setSavedMsg(t('common.saved'));
                      setTimeout(() => setSavedMsg(''), 2000);
                    }}
                  >
                    {t('settings.soulReject')}
                  </SettingsAction>
                </div>
              </div>
            ))}
          </div>
        )}

        <SettingsHint>{t('settings.soulPathHint')}</SettingsHint>
      </div>
    </SettingsSection>
  );
}
