interface DeptGlyphProps {
  deptKey: string;
  size?: number;
  stroke?: string;
}

export default function DeptGlyph({ deptKey, size = 16, stroke = '#8B7355' }: DeptGlyphProps) {
  const s = { width: size, height: size, viewBox: '0 0 16 16', fill: 'none', stroke, strokeWidth: 1.5 };

  switch (deptKey) {
    case 'neige':
      return <svg {...s}><rect x="2" y="2" width="12" height="3" rx="0.5" /><rect x="2" y="6.5" width="12" height="3" rx="0.5" /><rect x="2" y="11" width="12" height="3" rx="0.5" /></svg>;
    case 'zhongshuling':
      return <svg {...s}><line x1="3" y1="13" x2="8" y2="3" /><line x1="8" y1="3" x2="13" y2="13" /><circle cx="5" cy="10" r="0.8" /><circle cx="11" cy="10" r="0.8" /></svg>;
    case 'menxiashizhong':
      return <svg {...s}><circle cx="7.5" cy="5.5" r="3.5" /><line x1="11" y1="9" x2="14" y2="13" /></svg>;
    case 'shangshuling':
      return <svg {...s}><circle cx="8" cy="5" r="2" /><line x1="5.5" y1="8.5" x2="10.5" y2="8.5" /><line x1="6.5" y1="10" x2="9.5" y2="10" /><line x1="7" y1="11.5" x2="9" y2="11.5" /></svg>;
    case 'libushangshu':
      return <svg {...s}><line x1="3" y1="3" x2="13" y2="3" /><line x1="3" y1="6.5" x2="13" y2="6.5" /><line x1="3" y1="10" x2="13" y2="10" /><line x1="3" y1="13.5" x2="13" y2="13.5" /></svg>;
    case 'bingbushangshu':
      return <svg {...s}><path d="M8 1.5L3 5v3.5C3 11 5.5 14 8 14.5c2.5-.5 5-3.5 5-6V5L8 1.5z" /><line x1="8" y1="5" x2="8" y2="10" /><line x1="5.5" y1="7.5" x2="10.5" y2="7.5" /></svg>;
    case 'gongbushangshu':
      return <svg {...s}><rect x="5" y="2" width="6" height="4" rx="0.5" /><rect x="3" y="6" width="10" height="8" rx="0.5" /><line x1="6" y1="9" x2="10" y2="9" /><line x1="6" y1="11" x2="10" y2="11" /></svg>;
    case 'xingbushangshu':
      return <svg {...s}><line x1="2" y1="2" x2="14" y2="14" /><line x1="14" y1="2" x2="2" y2="14" /><circle cx="8" cy="8" r="2" /></svg>;
    case 'liburshangshu':
      return <svg {...s}><rect x="3" y="2" width="10" height="12" rx="0.5" /><path d="M5 4h6M5 6.5h6M5 9h3" /></svg>;
    default:
      return <svg {...s}><circle cx="8" cy="8" r="6" /></svg>;
  }
}
