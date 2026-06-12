import { useNavigate } from 'react-router-dom';

export default function SettingsMenu() {
  const navigate = useNavigate();
  return (
    <button
      onClick={() => navigate('/settings')}
      className="text-xs px-2 py-1 text-ink-400 hover:text-ink-100 hover:bg-ink-800 rounded"
    >
      ⚙ 设置
    </button>
  );
}
