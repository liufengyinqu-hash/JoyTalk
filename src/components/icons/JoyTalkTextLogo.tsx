import React from "react";

const JoyTalkTextLogo = ({
  width,
  height,
  className,
}: {
  width?: number;
  height?: number;
  className?: string;
}) => {
  return (
    <svg
      width={width}
      height={height}
      className={className}
      viewBox="0 0 360 96"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      <defs>
        <linearGradient id="jt-text-grad" x1="0" y1="0" x2="1" y2="0">
          <stop offset="0%" stopColor="#22D3EE" />
          <stop offset="100%" stopColor="#0891B2" />
        </linearGradient>
      </defs>
      <text
        x="0"
        y="70"
        fontFamily="ui-rounded, 'SF Pro Rounded', 'Segoe UI Rounded', system-ui, sans-serif"
        fontWeight="800"
        fontSize="72"
        letterSpacing="-2"
        fill="url(#jt-text-grad)"
      >
        JoyTalk
      </text>
    </svg>
  );
};

export default JoyTalkTextLogo;
