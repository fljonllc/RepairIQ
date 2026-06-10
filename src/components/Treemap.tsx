import { useState } from "react";
import type { ScannedItem, SafetyLevel } from "../types";
import { formatBytes } from "../utils";

interface TreemapProps {
  items: ScannedItem[];
  onItemClick: (item: ScannedItem) => void;
}

const SAFETY_BG: Record<SafetyLevel, string> = {
  Safe: "rgba(16, 185, 129, 0.6)",
  Review: "rgba(245, 158, 11, 0.5)",
  Archive: "rgba(99, 102, 241, 0.5)",
  Protected: "rgba(107, 114, 128, 0.4)",
};

const SAFETY_BORDER: Record<SafetyLevel, string> = {
  Safe: "rgba(16, 185, 129, 0.9)",
  Review: "rgba(245, 158, 11, 0.8)",
  Archive: "rgba(99, 102, 241, 0.8)",
  Protected: "rgba(107, 114, 128, 0.7)",
};

interface TreemapRect {
  item: ScannedItem;
  x: number;
  y: number;
  width: number;
  height: number;
}

function squarify(
  items: ScannedItem[],
  containerWidth: number,
  containerHeight: number
): TreemapRect[] {
  if (items.length === 0) return [];

  const totalSize = items.reduce((sum, item) => sum + item.size_bytes, 0);
  if (totalSize === 0) return [];

  // Simple slice-and-dice treemap
  const rects: TreemapRect[] = [];
  let x = 0;
  let y = 0;
  let remainingWidth = containerWidth;
  let remainingHeight = containerHeight;
  let isHorizontal = containerWidth >= containerHeight;

  const sorted = [...items].sort((a, b) => b.size_bytes - a.size_bytes);

  for (const item of sorted) {
    const ratio = item.size_bytes / totalSize;

    let rectWidth: number;
    let rectHeight: number;

    if (isHorizontal) {
      rectWidth = remainingWidth * ratio * (containerHeight / remainingHeight);
      rectWidth = Math.min(rectWidth, remainingWidth);
      rectHeight = remainingHeight;

      if (rectWidth > remainingWidth * 0.6) {
        rectWidth = remainingWidth;
        rectHeight = remainingHeight * ratio * (containerWidth / remainingWidth);
        rectHeight = Math.min(rectHeight, remainingHeight);

        rects.push({ item, x, y, width: rectWidth, height: rectHeight });
        y += rectHeight;
        remainingHeight -= rectHeight;
        isHorizontal = remainingWidth >= remainingHeight;
      } else {
        rects.push({ item, x, y, width: rectWidth, height: rectHeight });
        x += rectWidth;
        remainingWidth -= rectWidth;
        isHorizontal = remainingWidth >= remainingHeight;
      }
    } else {
      rectHeight = remainingHeight * ratio * (containerWidth / remainingWidth);
      rectHeight = Math.min(rectHeight, remainingHeight);
      rectWidth = remainingWidth;

      if (rectHeight > remainingHeight * 0.6) {
        rectHeight = remainingHeight;
        rectWidth = remainingWidth * ratio * (containerHeight / remainingHeight);
        rectWidth = Math.min(rectWidth, remainingWidth);

        rects.push({ item, x, y, width: rectWidth, height: rectHeight });
        x += rectWidth;
        remainingWidth -= rectWidth;
        isHorizontal = remainingWidth >= remainingHeight;
      } else {
        rects.push({ item, x, y, width: rectWidth, height: rectHeight });
        y += rectHeight;
        remainingHeight -= rectHeight;
        isHorizontal = remainingWidth >= remainingHeight;
      }
    }
  }

  return rects;
}

export function Treemap({ items, onItemClick }: TreemapProps) {
  const [hoveredPath, setHoveredPath] = useState<string | null>(null);

  const containerWidth = 800;
  const containerHeight = 400;

  // Take top 30 items for readability
  const topItems = items.slice(0, 30);
  const rects = squarify(topItems, containerWidth, containerHeight);

  return (
    <div className="treemap-container">
      <div className="treemap-legend">
        <span className="treemap-legend-item">
          <span className="legend-dot" style={{ background: SAFETY_BG.Safe }} /> Safe
        </span>
        <span className="treemap-legend-item">
          <span className="legend-dot" style={{ background: SAFETY_BG.Review }} /> Review
        </span>
        <span className="treemap-legend-item">
          <span className="legend-dot" style={{ background: SAFETY_BG.Archive }} /> Archive
        </span>
        <span className="treemap-legend-item">
          <span className="legend-dot" style={{ background: SAFETY_BG.Protected }} /> Protected
        </span>
      </div>
      <svg
        viewBox={`0 0 ${containerWidth} ${containerHeight}`}
        className="treemap-svg"
        preserveAspectRatio="xMidYMid meet"
      >
        {rects.map((rect) => {
          const isHovered = hoveredPath === rect.item.path;
          const minDim = Math.min(rect.width, rect.height);
          const showLabel = minDim > 30;

          return (
            <g
              key={rect.item.path}
              onClick={() => onItemClick(rect.item)}
              onMouseEnter={() => setHoveredPath(rect.item.path)}
              onMouseLeave={() => setHoveredPath(null)}
              style={{ cursor: "pointer" }}
            >
              <rect
                x={rect.x + 1}
                y={rect.y + 1}
                width={Math.max(0, rect.width - 2)}
                height={Math.max(0, rect.height - 2)}
                fill={SAFETY_BG[rect.item.safety]}
                stroke={isHovered ? "#fff" : SAFETY_BORDER[rect.item.safety]}
                strokeWidth={isHovered ? 2 : 1}
                rx={3}
                opacity={isHovered ? 1 : 0.85}
              />
              {showLabel && (
                <>
                  <text
                    x={rect.x + 6}
                    y={rect.y + 16}
                    fill="#fff"
                    fontSize={11}
                    fontWeight={600}
                    style={{ pointerEvents: "none" }}
                  >
                    {rect.item.name.length > Math.floor(rect.width / 7)
                      ? rect.item.name.slice(0, Math.floor(rect.width / 7)) + "…"
                      : rect.item.name}
                  </text>
                  {rect.height > 40 && (
                    <text
                      x={rect.x + 6}
                      y={rect.y + 32}
                      fill="rgba(255,255,255,0.7)"
                      fontSize={10}
                      style={{ pointerEvents: "none" }}
                    >
                      {formatBytes(rect.item.size_bytes)}
                    </text>
                  )}
                </>
              )}
            </g>
          );
        })}
      </svg>

      {/* Tooltip */}
      {hoveredPath && (() => {
        const item = topItems.find((i) => i.path === hoveredPath);
        if (!item) return null;
        return (
          <div className="treemap-tooltip">
            <strong>{item.name}</strong>
            <span>{formatBytes(item.size_bytes)}</span>
            <span className="tooltip-desc">{item.description}</span>
            <span className={`tooltip-safety safety-${item.safety.toLowerCase()}`}>
              {item.safety}
            </span>
          </div>
        );
      })()}
    </div>
  );
}
