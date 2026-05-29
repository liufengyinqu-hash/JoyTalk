import React from "react";

const JoyTalkLogo = ({
  width,
  height,
  className,
  ...rest
}: {
  width?: number | string;
  height?: number | string;
  className?: string;
  [key: string]: unknown;
}) => (
  <svg
    width={width || 24}
    height={height || 24}
    viewBox="0 0 64 64"
    className={className}
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    {...rest}
  >
    <defs>
      <linearGradient id="jt-mini" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0%" stopColor="#22D3EE" />
        <stop offset="100%" stopColor="#0891B2" />
      </linearGradient>
    </defs>
    {/* Bubble */}
    <path
      d="M10 12 H50 a8 8 0 0 1 8 8 v18 a8 8 0 0 1 -8 8 H30 l-12 10 v-10 H10 a8 8 0 0 1 -8 -8 V20 a8 8 0 0 1 8 -8 z"
      fill="none"
      stroke="url(#jt-mini)"
      strokeWidth="2.5"
    />
    {/* 3 dots */}
    <circle cx="20" cy="29" r="3" fill="url(#jt-mini)" />
    <circle cx="32" cy="29" r="3" fill="url(#jt-mini)" />
    <circle cx="44" cy="29" r="3" fill="url(#jt-mini)" />
    {/* Mic */}
    <rect x="29" y="36" width="6" height="6" rx="2" fill="url(#jt-mini)" />
  </svg>
);

export default JoyTalkLogo;
