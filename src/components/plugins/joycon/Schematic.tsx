import React from "react";
import type { ControllerKind, JoyConButton } from "./types";

export type SchematicLayout = "left" | "right" | "pro" | "pair";

interface Props {
  layout: SchematicLayout;
  highlight?: JoyConButton[];
  active?: JoyConButton[];
  labels?: Partial<Record<JoyConButton, string>>;
  onButtonClick?: (b: JoyConButton) => void;
  className?: string;
  /** Controller kinds that are currently connected. Each matching controller
   *  in the schematic gets a breathing accent-colored glow around its body. */
  connectedKinds?: ControllerKind[];
}

interface SideTheme {
  /** 已映射按键的外圈颜色 */
  highlightStroke: string;
  /** 按键下方映射说明文字 */
  labelFill: string;
  /** 连接呼吸灯内圈描边（红白/白蓝交替） */
  glowBreathStroke: string;
  /** 机身上方提亮层 */
  pulseFill: string;
  pulseOpacity: string;
  /** 外圈光晕透明度脉动 */
  glowOuterOpacity: string;
  /** 内圈描边透明度脉动 */
  glowInnerOpacity: string;
  /** 机身淡色层（右手柄红）透明度脉动，无则省略 */
  bodyTintOpacity?: string;
}

const JOYCON_L_BODY = "#00b9e8";
const JOYCON_R_BODY = "#ff4f3a";

/** 手柄示意图用独立配色，避免全局 accent（青色）贴在蓝/红机身上对比度不足。 */
const SIDE_THEMES: Record<ControllerKind, SideTheme> = {
  joy_con_left: {
    highlightStroke: "#ffffff",
    labelFill: "#ffffff",
    glowBreathStroke: `#ffffff;${JOYCON_L_BODY};#ffffff`,
    pulseFill: "#ffffff",
    pulseOpacity: "0.06;0.28;0.06",
    glowOuterOpacity: "0.25;1;0.25",
    glowInnerOpacity: "0.5;1;0.5",
  },
  joy_con_right: {
    highlightStroke: "#fff8e7",
    labelFill: "#fff8e7",
    glowBreathStroke: `#ffffff;${JOYCON_R_BODY};#ffffff`,
    pulseFill: "#ffffff",
    pulseOpacity: "0.04;0.14;0.04",
    glowOuterOpacity: "0.12;0.42;0.12",
    glowInnerOpacity: "0.18;0.4;0.18",
    bodyTintOpacity: "0;0.1;0",
  },
  pro_controller: {
    highlightStroke: "var(--color-accent)",
    labelFill: "var(--color-accent)",
    glowBreathStroke: "var(--color-accent);#ffffff;var(--color-accent)",
    pulseFill: "#ffffff",
    pulseOpacity: "0.06;0.22;0.06",
    glowOuterOpacity: "0.25;1;0.25",
    glowInnerOpacity: "0.5;1;0.5",
  },
  unknown: {
    highlightStroke: "var(--color-accent)",
    labelFill: "var(--color-accent)",
    glowBreathStroke: "var(--color-accent);#ffffff;var(--color-accent)",
    pulseFill: "#ffffff",
    pulseOpacity: "0.06;0.22;0.06",
    glowOuterOpacity: "0.25;1;0.25",
    glowInnerOpacity: "0.5;1;0.5",
  },
};

interface BtnDef {
  id: JoyConButton;
  label: string;
  shape: "rect" | "circle" | "pill";
  x: number;
  y: number;
  w?: number;
  h?: number;
  r?: number;
}

interface LayoutDef {
  width: number;
  height: number;
  bodyColor: string;
  /** Which controller this layout represents, so the connected-glow can be
   *  applied only to the controllers that are actually connected. */
  kind: ControllerKind;
  body: { x: number; y: number; w: number; h: number; rx: number };
  buttons: BtnDef[];
}

// Joy-Con (L), held vertically: neon-blue body, analog stick on top, the
// four D-pad buttons below, minus button, square Capture button at the
// bottom, and the SL/SR rail on the inner (right) edge.
const LEFT_LAYOUT: LayoutDef = {
  width: 150,
  height: 360,
  bodyColor: JOYCON_L_BODY,
  kind: "joy_con_left",
  body: { x: 14, y: 14, w: 122, h: 332, rx: 30 },
  buttons: [
    { id: "l", label: "L", shape: "pill", x: 28, y: 6, w: 94, h: 14 },
    { id: "zl", label: "ZL", shape: "pill", x: 28, y: 24, w: 94, h: 14 },
    { id: "l_stick", label: "", shape: "circle", x: 75, y: 100, r: 26 },
    { id: "l_stick_up", label: "", shape: "circle", x: 75, y: 68, r: 5 },
    { id: "l_stick_down", label: "", shape: "circle", x: 75, y: 132, r: 5 },
    { id: "l_stick_left", label: "", shape: "circle", x: 43, y: 100, r: 5 },
    { id: "l_stick_right", label: "", shape: "circle", x: 107, y: 100, r: 5 },
    { id: "minus", label: "−", shape: "circle", x: 112, y: 50, r: 8 },
    { id: "up", label: "↑", shape: "circle", x: 75, y: 208, r: 13 },
    { id: "left", label: "←", shape: "circle", x: 45, y: 238, r: 13 },
    { id: "right", label: "→", shape: "circle", x: 105, y: 238, r: 13 },
    { id: "down", label: "↓", shape: "circle", x: 75, y: 268, r: 13 },
    { id: "capture", label: "◰", shape: "circle", x: 75, y: 312, r: 10 },
    { id: "sl_left", label: "SL", shape: "rect", x: 130, y: 110, w: 8, h: 58 },
    { id: "sr_left", label: "SR", shape: "rect", x: 130, y: 178, w: 8, h: 58 },
  ],
};

// Joy-Con (R), held vertically: neon-red body, ABXY diamond on top, analog
// stick below, plus button, IR window above Home at the bottom, NFC on R stick.
const RIGHT_LAYOUT: LayoutDef = {
  width: 150,
  height: 360,
  bodyColor: JOYCON_R_BODY,
  kind: "joy_con_right",
  body: { x: 14, y: 14, w: 122, h: 332, rx: 30 },
  buttons: [
    { id: "r", label: "R", shape: "pill", x: 28, y: 6, w: 94, h: 14 },
    { id: "zr", label: "ZR", shape: "pill", x: 28, y: 24, w: 94, h: 14 },
    { id: "x", label: "X", shape: "circle", x: 75, y: 70, r: 13 },
    { id: "y", label: "Y", shape: "circle", x: 45, y: 100, r: 13 },
    { id: "a", label: "A", shape: "circle", x: 105, y: 100, r: 13 },
    { id: "b", label: "B", shape: "circle", x: 75, y: 130, r: 13 },
    { id: "plus", label: "+", shape: "circle", x: 38, y: 50, r: 8 },
    { id: "r_stick", label: "", shape: "circle", x: 75, y: 216, r: 26 },
    { id: "r_stick_up", label: "", shape: "circle", x: 75, y: 184, r: 5 },
    { id: "r_stick_down", label: "", shape: "circle", x: 75, y: 248, r: 5 },
    { id: "r_stick_left", label: "", shape: "circle", x: 43, y: 216, r: 5 },
    { id: "r_stick_right", label: "", shape: "circle", x: 107, y: 216, r: 5 },
    // NFC 读卡区：右摇杆（贴卡位置）
    { id: "nfc_tag_present", label: "NFC", shape: "circle", x: 75, y: 216, r: 12 },
    // IR 接近感应：Home 键上方黑窗
    { id: "ir_proximity", label: "IR", shape: "circle", x: 52, y: 284, r: 7 },
    { id: "home", label: "⌂", shape: "circle", x: 75, y: 312, r: 10 },
    { id: "sl_right", label: "SL", shape: "rect", x: 12, y: 178, w: 8, h: 58 },
    { id: "sr_right", label: "SR", shape: "rect", x: 12, y: 110, w: 8, h: 58 },
  ],
};

const PRO_LAYOUT: LayoutDef = {
  width: 380,
  height: 220,
  bodyColor: "#2b2f36",
  kind: "pro_controller",
  body: { x: 10, y: 30, w: 360, h: 170, rx: 30 },
  buttons: [
    { id: "zl", label: "ZL", shape: "pill", x: 30, y: 8, w: 80, h: 16 },
    { id: "zr", label: "ZR", shape: "pill", x: 270, y: 8, w: 80, h: 16 },
    { id: "l", label: "L", shape: "pill", x: 50, y: 32, w: 60, h: 12 },
    { id: "r", label: "R", shape: "pill", x: 270, y: 32, w: 60, h: 12 },
    { id: "up", label: "↑", shape: "circle", x: 100, y: 90, r: 12 },
    { id: "left", label: "←", shape: "circle", x: 72, y: 118, r: 12 },
    { id: "right", label: "→", shape: "circle", x: 128, y: 118, r: 12 },
    { id: "down", label: "↓", shape: "circle", x: 100, y: 146, r: 12 },
    { id: "x", label: "X", shape: "circle", x: 290, y: 90, r: 12 },
    { id: "y", label: "Y", shape: "circle", x: 262, y: 118, r: 12 },
    { id: "a", label: "A", shape: "circle", x: 318, y: 118, r: 12 },
    { id: "b", label: "B", shape: "circle", x: 290, y: 146, r: 12 },
    { id: "l_stick", label: "", shape: "circle", x: 160, y: 110, r: 18 },
    { id: "r_stick", label: "", shape: "circle", x: 230, y: 150, r: 18 },
    { id: "minus", label: "−", shape: "circle", x: 162, y: 80, r: 8 },
    { id: "plus", label: "+", shape: "circle", x: 228, y: 80, r: 8 },
    { id: "home", label: "⌂", shape: "circle", x: 175, y: 170, r: 9 },
    { id: "capture", label: "◰", shape: "circle", x: 215, y: 170, r: 9 },
  ],
};

function getLayout(layout: SchematicLayout) {
  switch (layout) {
    case "left":
      return [{ ofx: 0, ofy: 0, def: LEFT_LAYOUT }];
    case "right":
      return [{ ofx: 0, ofy: 0, def: RIGHT_LAYOUT }];
    case "pair":
      return [
        { ofx: 0, ofy: 0, def: LEFT_LAYOUT },
        { ofx: 184, ofy: 0, def: RIGHT_LAYOUT },
      ];
    case "pro":
    default:
      return [{ ofx: 0, ofy: 0, def: PRO_LAYOUT }];
  }
}

// IMU / motion gestures have no physical position on the controller, so they
// are rendered as a dedicated badge row under the schematic. They reuse the
// same `active`/`highlight`/`onButtonClick` plumbing as physical buttons, so
// firing a gesture lights its badge blue just like pressing a button.
const GESTURE_DEFS: { id: JoyConButton; label: string; full: string }[] = [
  { id: "shake", label: "震", full: "摇晃" },
  { id: "flip_up", label: "翻↑", full: "上翻" },
  { id: "flip_down", label: "翻↓", full: "下翻" },
  { id: "tilt_left", label: "↺", full: "左倾" },
  { id: "tilt_right", label: "↻", full: "右倾" },
  { id: "shake_horizontal", label: "↔", full: "横晃" },
  { id: "shake_vertical", label: "↕", full: "竖晃" },
];

const GBADGE_W = 36;
const GBADGE_H = 22;
const GBADGE_GAP = 6;
const GESTURE_ROW_W =
  GESTURE_DEFS.length * GBADGE_W + (GESTURE_DEFS.length - 1) * GBADGE_GAP;
const GESTURE_TOP_GAP = 14;
const GESTURE_HEADER_H = 16;
const GESTURE_LABEL_H = 12;
const GESTURE_BLOCK_H =
  GESTURE_TOP_GAP + GESTURE_HEADER_H + GBADGE_H + GESTURE_LABEL_H;

export const Schematic: React.FC<Props> = ({
  layout,
  highlight = [],
  active = [],
  labels = {},
  onButtonClick,
  className,
  connectedKinds = [],
}) => {
  const parts = getLayout(layout);
  const contentW = parts.reduce(
    (acc, p) => Math.max(acc, p.ofx + p.def.width),
    0,
  );
  const contentH = parts.reduce(
    (acc, p) => Math.max(acc, p.ofy + p.def.height),
    0,
  );
  const totalW = Math.max(contentW, GESTURE_ROW_W + 8);
  const totalH = contentH + GESTURE_BLOCK_H;

  return (
    <svg
      viewBox={`0 0 ${totalW} ${totalH}`}
      className={`max-w-full ${className ?? ""}`}
      xmlns="http://www.w3.org/2000/svg"
    >
      <defs>
        <filter
          id="jc-glow-strong"
          x="-100%"
          y="-100%"
          width="300%"
          height="300%"
        >
          <feGaussianBlur in="SourceGraphic" stdDeviation="8" result="blur1" />
          <feGaussianBlur in="SourceGraphic" stdDeviation="16" result="blur2" />
          <feMerge>
            <feMergeNode in="blur2" />
            <feMergeNode in="blur1" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>
      {parts.map((part, idx) => {
        const { def, ofx, ofy } = part;
        const theme = SIDE_THEMES[def.kind];
        const isConnected = connectedKinds.includes(def.kind);
        const glowPad = 8;
        return (
          <g
            key={idx}
            transform={`translate(${ofx}, ${ofy})`}
            opacity={isConnected ? 1 : 0.42}
          >
            {/* 外层光晕在机身后面，露出边缘一圈 */}
            {isConnected && (
              <rect
                x={def.body.x - glowPad}
                y={def.body.y - glowPad}
                width={def.body.w + glowPad * 2}
                height={def.body.h + glowPad * 2}
                rx={def.body.rx + 6}
                fill="none"
                stroke="#ffffff"
                strokeWidth={6}
                filter="url(#jc-glow-strong)"
              >
                <animate
                  attributeName="opacity"
                  values={theme.glowOuterOpacity}
                  dur="1.4s"
                  repeatCount="indefinite"
                />
                <animate
                  attributeName="stroke-width"
                  values="4;10;4"
                  dur="1.4s"
                  repeatCount="indefinite"
                />
              </rect>
            )}
            <rect
              x={def.body.x}
              y={def.body.y}
              width={def.body.w}
              height={def.body.h}
              rx={def.body.rx}
              fill={def.bodyColor}
              stroke="rgba(0,0,0,0.28)"
              strokeWidth={2}
            />
            {/* 内圈 + 机身提亮叠在机身上面，红白/白蓝交替更明显 */}
            {isConnected && (
              <>
                <rect
                  x={def.body.x - 2}
                  y={def.body.y - 2}
                  width={def.body.w + 4}
                  height={def.body.h + 4}
                  rx={def.body.rx + 2}
                  fill="none"
                  stroke="#ffffff"
                  strokeWidth={5}
                  filter="url(#jc-glow-strong)"
                  pointerEvents="none"
                >
                  <animate
                    attributeName="stroke"
                    values={theme.glowBreathStroke}
                    dur="1.4s"
                    repeatCount="indefinite"
                  />
                  <animate
                    attributeName="opacity"
                    values={theme.glowInnerOpacity}
                    dur="1.4s"
                    repeatCount="indefinite"
                  />
                  <animate
                    attributeName="stroke-width"
                    values="3;7;3"
                    dur="1.4s"
                    repeatCount="indefinite"
                  />
                </rect>
                <rect
                  x={def.body.x}
                  y={def.body.y}
                  width={def.body.w}
                  height={def.body.h}
                  rx={def.body.rx}
                  fill={theme.pulseFill}
                  pointerEvents="none"
                >
                  <animate
                    attributeName="opacity"
                    values={theme.pulseOpacity}
                    dur="1.4s"
                    repeatCount="indefinite"
                  />
                </rect>
                {theme.bodyTintOpacity && (
                  <rect
                    x={def.body.x}
                    y={def.body.y}
                    width={def.body.w}
                    height={def.body.h}
                    rx={def.body.rx}
                    fill={def.bodyColor}
                    pointerEvents="none"
                  >
                    <animate
                      attributeName="opacity"
                      values={theme.bodyTintOpacity}
                      dur="1.4s"
                      repeatCount="indefinite"
                    />
                  </rect>
                )}
              </>
            )}
            {def.buttons.map((b) => {
              const isHighlight = highlight.includes(b.id);
              const isActive = active.includes(b.id);
              // On the colored Joy-Con body: buttons are dark/translucent by
              // default, light up white when pressed (active), and get an accent
              // outline when they have a mapping (highlight).
              const fill = isActive ? "#ffffff" : "rgba(0,0,0,0.32)";
              const stroke = isHighlight
                ? theme.highlightStroke
                : "rgba(255,255,255,0.65)";
              const strokeW = isHighlight ? 2.5 : 1.2;
              const cursor = onButtonClick ? "cursor-pointer" : "";
              const handleClick = () => onButtonClick?.(b.id);
              const txtFill = isActive ? "#111827" : "rgba(255,255,255,0.95)";
              const lblFill = theme.labelFill;
              const lbl = labels[b.id];
              if (b.shape === "circle") {
                return (
                  <g
                    key={b.id}
                    className={cursor}
                    onClick={handleClick}
                  >
                    <circle
                      cx={b.x}
                      cy={b.y}
                      r={b.r}
                      fill={fill}
                      stroke={stroke}
                      strokeWidth={strokeW}
                    />
                    <text
                      x={b.x}
                      y={b.y + 4}
                      textAnchor="middle"
                      fontSize={10}
                      fontWeight={600}
                      fill={txtFill}
                    >
                      {b.label}
                    </text>
                    {lbl && (
                      <text
                        x={b.x}
                        y={b.y + (b.r ?? 8) + 12}
                        textAnchor="middle"
                        fontSize={8}
                        fontWeight={600}
                        fill={lblFill}
                        stroke="rgba(0,0,0,0.35)"
                        strokeWidth={0.6}
                        paintOrder="stroke"
                      >
                        {lbl}
                      </text>
                    )}
                  </g>
                );
              }
              const w = b.w ?? 30;
              const h = b.h ?? 14;
              return (
                <g key={b.id} className={cursor} onClick={handleClick}>
                  <rect
                    x={b.x}
                    y={b.y}
                    width={w}
                    height={h}
                    rx={b.shape === "pill" ? h / 2 : 3}
                    fill={fill}
                    stroke={stroke}
                    strokeWidth={strokeW}
                  />
                  <text
                    x={b.x + w / 2}
                    y={b.y + h / 2 + 3}
                    textAnchor="middle"
                    fontSize={9}
                    fontWeight={600}
                    fill={txtFill}
                  >
                    {b.label}
                  </text>
                  {lbl && (
                    <text
                      x={b.x + w / 2}
                      y={b.y + h + 10}
                      textAnchor="middle"
                      fontSize={8}
                      fontWeight={600}
                      fill={lblFill}
                      stroke="rgba(0,0,0,0.35)"
                      strokeWidth={0.6}
                      paintOrder="stroke"
                    >
                      {lbl}
                    </text>
                  )}
                </g>
              );
            })}
          </g>
        );
      })}
      {(() => {
        const rowX = (totalW - GESTURE_ROW_W) / 2;
        const headerY = contentH + GESTURE_TOP_GAP;
        const badgeY = headerY + GESTURE_HEADER_H;
        return (
          <g>
            <text
              x={totalW / 2}
              y={headerY + 2}
              textAnchor="middle"
              fontSize={9}
              fill="var(--color-text-secondary)"
            >
              体感手势
            </text>
            {GESTURE_DEFS.map((g, i) => {
              const x = rowX + i * (GBADGE_W + GBADGE_GAP);
              const isActive = active.includes(g.id);
              const isHighlight = highlight.includes(g.id);
              const lbl = labels[g.id];
              return (
                <g
                  key={g.id}
                  className={onButtonClick ? "cursor-pointer" : ""}
                  onClick={() => onButtonClick?.(g.id)}
                >
                  <title>{g.full}</title>
                  <rect
                    x={x}
                    y={badgeY}
                    width={GBADGE_W}
                    height={GBADGE_H}
                    rx={GBADGE_H / 2}
                    fill={isActive ? "var(--color-accent)" : "var(--color-surface)"}
                    stroke={
                      isHighlight
                        ? "var(--color-accent)"
                        : "var(--color-border-strong)"
                    }
                    strokeWidth={isHighlight ? 2 : 1}
                  />
                  <text
                    x={x + GBADGE_W / 2}
                    y={badgeY + GBADGE_H / 2 + 3}
                    textAnchor="middle"
                    fontSize={9}
                    fontWeight={600}
                    fill={isActive ? "white" : "var(--color-text-secondary)"}
                  >
                    {g.label}
                  </text>
                  {lbl && (
                    <text
                      x={x + GBADGE_W / 2}
                      y={badgeY + GBADGE_H + 9}
                      textAnchor="middle"
                      fontSize={7}
                      fill="var(--color-accent)"
                    >
                      {lbl}
                    </text>
                  )}
                </g>
              );
            })}
          </g>
        );
      })()}
    </svg>
  );
};
