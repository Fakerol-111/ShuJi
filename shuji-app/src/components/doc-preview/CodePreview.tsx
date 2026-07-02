import { useTranslation } from 'react-i18next';
import { basenameFromPath } from '../../utils/pathBasename';

function languageName(path: string) {
  const ext = path.split('.').pop()?.toLowerCase();
  const map: Record<string, string> = {
    rs: 'Rust',
    ts: 'TypeScript',
    tsx: 'TSX',
    js: 'JavaScript',
    jsx: 'JSX',
    json: 'JSON',
    jsonl: 'JSONL',
    toml: 'TOML',
    yaml: 'YAML',
    yml: 'YAML',
    css: 'CSS',
    html: 'HTML',
    py: 'Python',
    sh: 'Shell',
    ps1: 'PowerShell',
    svg: 'SVG',
    txt: 'Text',
    env: 'Env',
  };
  return ext ? map[ext] || ext.toUpperCase() : 'Text';
}

function fileGlyph(path: string) {
  const ext = path.split('.').pop()?.toLowerCase();
  if (['ts', 'tsx', 'js', 'jsx'].includes(ext || '')) return 'TS';
  if (ext === 'rs') return 'RS';
  if (['json', 'jsonl'].includes(ext || '')) return '{}';
  if (['toml', 'yaml', 'yml', 'env'].includes(ext || '')) return '⚙';
  if (ext === 'py') return 'PY';
  return 'TXT';
}

export default function CodePreview({
  content,
  path,
  openLineLabel,
  onOpenLine,
}: {
  content: string;
  path: string;
  openLineLabel?: string;
  onOpenLine?: (line: number) => void | Promise<void>;
}) {
  const { t } = useTranslation();
  const lines = (content || t('docPreview.fileEmpty')).split(/\r?\n/);
  const language = languageName(path);
  const lineClickable = Boolean(onOpenLine);

  return (
    <div
      className="doc-preview-code min-w-0 rounded-lg border overflow-hidden"
      style={{ borderColor: 'var(--code-border)', backgroundColor: 'var(--code-bg)' }}
    >
      <div
        className="h-8 flex items-center justify-between text-[11px] shrink-0 min-w-0"
        style={{
          backgroundColor: 'var(--code-tab-bg)',
          borderBottom: '1px solid var(--code-border)',
        }}
      >
        <div
          className="h-full px-3 flex items-center gap-2 font-mono min-w-0"
          style={{
            backgroundColor: 'var(--code-bg)',
            borderRight: '1px solid var(--code-border)',
            color: 'var(--code-text)',
          }}
        >
          <span style={{ color: 'var(--code-muted)' }}>{fileGlyph(path)}</span>
          <span className="truncate">{basenameFromPath(path)}</span>
        </div>
        <div
          className="px-3 font-mono flex items-center gap-3 shrink-0"
          style={{ color: 'var(--code-muted)' }}
        >
          <span>{language}</span>
          <span>{lines.length.toLocaleString()} lines</span>
          <span>{content.length.toLocaleString()} chars</span>
        </div>
      </div>
      <div className="doc-preview-code-scroll overflow-auto min-w-0 text-[13px] leading-[22px] font-[Cascadia_Code,JetBrains_Mono,Consolas,Menlo,Monaco,monospace]">
        <table className="w-full border-separate border-spacing-0">
          <tbody>
            {lines.map((line, index) => (
              <tr key={index} className="code-preview-row">
                <td
                  className={`select-none sticky left-0 w-14 min-w-14 pr-3 text-right align-top ${lineClickable ? 'cursor-pointer hover:text-vermillion hover:underline' : ''}`}
                  style={{
                    backgroundColor: 'var(--code-bg)',
                    color: 'var(--code-line-num)',
                    borderRight: '1px solid var(--code-border)',
                  }}
                  title={lineClickable ? openLineLabel : undefined}
                  onClick={lineClickable ? () => onOpenLine?.(index + 1) : undefined}
                >
                  {index + 1}
                </td>
                <td
                  className="pl-4 pr-6 whitespace-pre align-top"
                  style={{ color: 'var(--code-text)' }}
                >
                  {line || ' '}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
