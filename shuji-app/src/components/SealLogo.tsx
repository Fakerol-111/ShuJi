interface Props {
  size?: number;
}

export function SealLogo({ size = 32 }: Props) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 40 40"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-label="枢机印章"
    >
      <rect x="1" y="1" width="38" height="38" rx="6" stroke="currentColor" strokeWidth="2" fill="none" />
      <text
        x="20"
        y="27"
        textAnchor="middle"
        fontFamily="'Noto Serif SC', serif"
        fontSize="20"
        fontWeight="600"
        fill="currentColor"
      >
        枢
      </text>
    </svg>
  );
}
