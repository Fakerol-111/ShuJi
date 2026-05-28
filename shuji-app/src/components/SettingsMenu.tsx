import { getConfig, saveConfig } from "../api";

interface SettingsMenuProps {
  open: boolean;
  setOpen: (open: boolean) => void;
  cfgKey: string;
  cfgUrl: string;
  cfgModel: string;
  cfgMsg: string;
  setCfgKey: (value: string) => void;
  setCfgUrl: (value: string) => void;
  setCfgModel: (value: string) => void;
  setCfgMsg: (value: string) => void;
}

export default function SettingsMenu(props: SettingsMenuProps) {
  const { open, setOpen, cfgKey, cfgUrl, cfgModel, cfgMsg, setCfgKey, setCfgUrl, setCfgModel, setCfgMsg } = props;
  const toggle = () => {
    setOpen(!open);
    if (!open) {
      getConfig().then((cfg) => {
        const d = cfg.roles?.default;
        if (d) { setCfgKey(d.api_key); setCfgUrl(d.api_url); setCfgModel(d.model); }
      }).catch((e) => console.error("读取配置失败:", e));
    }
  };

  return (
    <div className="relative">
      <button onClick={toggle} className="text-xs px-2 py-1 text-ink-400 hover:text-ink-100 hover:bg-ink-800 rounded">⚙ 设置</button>
      {open && (
        <div className="absolute right-0 top-full mt-1 w-72 bg-ink-900 border border-ink-700 rounded-lg shadow-xl z-50 p-3 space-y-2">
          <ConfigInput label="API 密钥" type="password" value={cfgKey} onChange={setCfgKey} />
          <ConfigInput label="API URL" value={cfgUrl} onChange={setCfgUrl} />
          <ConfigInput label="模型" value={cfgModel} onChange={setCfgModel} />
          <div className="flex gap-2 items-center">
            <button onClick={async () => {
              try {
                await saveConfig({ roles: { default: { api_key: cfgKey, api_url: cfgUrl, model: cfgModel } } });
                setCfgMsg("已保存");
                setTimeout(() => setCfgMsg(""), 1500);
              } catch (e) { setCfgMsg(String(e)); }
            }} className="text-xs px-3 py-1 bg-ink-700 text-ink-200 rounded hover:bg-ink-600">保存</button>
            {cfgMsg && <span className={`text-[10px] ${cfgMsg === "已保存" ? "text-green-400" : "text-red-400"}`}>{cfgMsg}</span>}
          </div>
        </div>
      )}
    </div>
  );
}

function ConfigInput({ label, value, onChange, type = "text" }: { label: string; value: string; onChange: (value: string) => void; type?: string }) {
  return (
    <label className="block">
      <span className="text-[10px] text-ink-500">{label}</span>
      <input type={type} value={value} onChange={(e) => onChange(e.target.value)} className="w-full mt-0.5 px-2 py-1 text-xs bg-ink-800 border border-ink-700 rounded text-ink-200 focus:outline-none focus:border-ink-500" />
    </label>
  );
}
