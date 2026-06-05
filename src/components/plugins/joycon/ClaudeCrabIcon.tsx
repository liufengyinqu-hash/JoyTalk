import React from "react";

/** Claude Code–style crab mascot (simplified SVG). */
export const ClaudeCrabIcon: React.FC<{
  className?: string;
  size?: number;
}> = ({ className, size = 72 }) => (
  <svg
    width={size}
    height={size}
    viewBox="0 0 64 64"
    className={className}
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    aria-hidden
  >
    <ellipse cx="32" cy="38" rx="18" ry="14" fill="#E07A5F" />
    <ellipse cx="32" cy="36" rx="14" ry="10" fill="#F4A261" />
    <path
      d="M14 28c-6-4-8-10-4-14 3-3 8-1 10 4M50 28c6-4 8-10 4-14-3-3-8-1-10 4"
      stroke="#C8553D"
      strokeWidth="3"
      strokeLinecap="round"
      fill="#E07A5F"
    />
    <path
      d="M10 34c-4 2-6 6-4 9 2 2 6 1 8-2M54 34c4 2 6 6 4 9-2 2-6 1-8-2"
      stroke="#C8553D"
      strokeWidth="2.5"
      strokeLinecap="round"
      fill="#E07A5F"
    />
    <circle cx="26" cy="34" r="3.5" fill="#1C1C1E" />
    <circle cx="38" cy="34" r="3.5" fill="#1C1C1E" />
    <circle cx="27" cy="33" r="1.2" fill="#FFFFFF" />
    <circle cx="39" cy="33" r="1.2" fill="#FFFFFF" />
    <path
      d="M22 44l-4 6M28 46l-2 5M36 46l2 5M42 44l4 6"
      stroke="#C8553D"
      strokeWidth="2"
      strokeLinecap="round"
    />
    <path
      d="M29 40c1 1 5 1 6 0"
      stroke="#C8553D"
      strokeWidth="1.5"
      strokeLinecap="round"
    />
  </svg>
);
