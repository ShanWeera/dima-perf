/**
 * FeatureTrack - Renders a single category track row in the SVG.
 *
 * Pure presentational component wrapped in React.memo. Renders feature
 * shapes (rounded rects for ranges, circles for points) with hover/select
 * styling. Uses color AND shape to distinguish categories (accessible).
 */

import React, { useMemo } from 'react';
import type { MappedFeature, FeatureCategoryConfig } from '@/lib/types';

/**
 * Determine a readable text color (white or black) based on the perceived
 * luminance of the background color. Uses the WCAG relative luminance formula.
 */
function contrastLabelColor(hexColor: string): string {
  const hex = hexColor.replace('#', '');
  if (hex.length < 6) return 'white';
  const r = parseInt(hex.slice(0, 2), 16) / 255;
  const g = parseInt(hex.slice(2, 4), 16) / 255;
  const b = parseInt(hex.slice(4, 6), 16) / 255;
  // sRGB to linear
  const toLinear = (c: number) => c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  const luminance = 0.2126 * toLinear(r) + 0.7152 * toLinear(g) + 0.0722 * toLinear(b);
  return luminance > 0.4 ? '#1f2937' : 'white';
}

interface FeatureTrackProps {
  /** Category key */
  category: string;
  /** Category display config */
  config: FeatureCategoryConfig;
  /** Features in this category (already filtered to mappable ones) */
  features: MappedFeature[];
  /** Start of the visible viewport in MSA position space */
  viewSeqStart: number;
  /** Span of the visible viewport in MSA position space */
  viewSeqSpan: number;
  /** SVG width in pixels */
  svgWidth: number;
  /** Y offset for this track row */
  yOffset: number;
  /** Track height */
  trackHeight: number;
  /** Left margin for labels */
  labelWidth: number;
  /** Currently hovered feature */
  hoveredFeature: MappedFeature | null;
  /** Currently selected feature */
  selectedFeature: MappedFeature | null;
  /** Hover callback */
  onMouseEnter: (feature: MappedFeature) => void;
  /** Leave callback */
  onMouseLeave: () => void;
  /** Click callback */
  onClick: (feature: MappedFeature) => void;
}

// Check if two MappedFeatures are the same entity.
// Includes categoryKey to distinguish features from different categories
// that happen to share the same position, type, and description.
function isSameFeature(a: MappedFeature | null, b: MappedFeature): boolean {
  return (
    a !== null &&
    a.begin === b.begin &&
    a.end === b.end &&
    a.feature_type === b.feature_type &&
    a.description === b.description &&
    a.categoryKey === b.categoryKey
  );
}

export const FeatureTrack = React.memo(function FeatureTrack({
  category,
  config,
  features,
  viewSeqStart,
  viewSeqSpan,
  svgWidth,
  yOffset,
  trackHeight,
  labelWidth,
  hoveredFeature,
  selectedFeature,
  onMouseEnter,
  onMouseLeave,
  onClick,
}: FeatureTrackProps) {
  const drawWidth = svgWidth - labelWidth;
  const midY = yOffset + trackHeight / 2;

  // Position → pixel mapping (viewport-aware)
  const posToX = (pos: number) =>
    labelWidth + ((pos - viewSeqStart) / viewSeqSpan) * drawWidth;

  // Viewport culling: only render features that intersect the visible range.
  // A small buffer (5% of span on each side) prevents pop-in at edges during pan.
  const viewSeqEnd = viewSeqStart + viewSeqSpan;
  const buffer = viewSeqSpan * 0.05;
  const cullStart = viewSeqStart - buffer;
  const cullEnd = viewSeqEnd + buffer;

  const visibleFeatures = useMemo(() => {
    return features.filter((f) => {
      if (f.msaBegin === null || f.msaEnd === null) return false;
      return f.msaEnd >= cullStart && f.msaBegin <= cullEnd;
    });
  }, [features, cullStart, cullEnd]);

  return (
    <g>
      {/* Track label */}
      <text
        x={labelWidth - 6}
        y={midY}
        textAnchor="end"
        dominantBaseline="central"
        className="fill-muted-foreground"
        fontSize={10}
      >
        {config.label}
      </text>

      {/* Track background line */}
      <line
        x1={labelWidth}
        y1={midY}
        x2={svgWidth}
        y2={midY}
        stroke="currentColor"
        strokeOpacity={0.06}
        strokeWidth={1}
      />

      {/* Feature shapes (viewport-culled for performance with large proteins) */}
      {visibleFeatures.map((f, i) => {
        // msaBegin/msaEnd are guaranteed non-null by the visibleFeatures filter
        const begin = f.msaBegin!;
        const end = f.msaEnd!;

        const isHovered = isSameFeature(hoveredFeature, f);
        const isSelected = isSameFeature(selectedFeature, f);
        const isPoint = begin === end;

        const x = posToX(begin);
        // +1 for inclusive end position: feature spans [msaBegin, msaEnd]
        const w = Math.max(posToX(end + 1) - x, 2);
        const fillOpacity = isHovered || isSelected ? 0.9 : 0.65;
        const strokeWidth = isSelected ? 2 : isHovered ? 1.5 : 0.5;

        // Common event handlers
        const handlers = {
          onMouseEnter: () => onMouseEnter(f),
          onMouseLeave: () => onMouseLeave(),
          onClick: () => onClick(f),
        };

        if (isPoint || config.shape === 'circle') {
          // Point features: circles
          const cx = x + w / 2;
          return (
            <g key={`${category}-${i}`}>
              <circle
                cx={cx}
                cy={midY}
                r={isHovered || isSelected ? 5 : 4}
                fill={config.color}
                fillOpacity={fillOpacity}
                stroke={config.color}
                strokeWidth={strokeWidth}
                className="cursor-pointer transition-all"
                {...handlers}
              />
              {/* Selection ring */}
              {isSelected && (
                <circle
                  cx={cx}
                  cy={midY}
                  r={7}
                  fill="none"
                  stroke={config.color}
                  strokeWidth={1.5}
                  strokeDasharray="2 2"
                  className="pointer-events-none"
                />
              )}
            </g>
          );
        }

        // Range features: rounded rectangles
        const rectH = isHovered || isSelected ? trackHeight - 4 : trackHeight - 6;
        const rectY = midY - rectH / 2;

        return (
          <g key={`${category}-${i}`}>
            <rect
              x={x}
              y={rectY}
              width={w}
              height={rectH}
              rx={3}
              fill={config.color}
              fillOpacity={fillOpacity}
              stroke={config.color}
              strokeWidth={strokeWidth}
              className="cursor-pointer transition-all"
              {...handlers}
            />
            {/* Selection ring */}
            {isSelected && (
              <rect
                x={x - 2}
                y={rectY - 2}
                width={w + 4}
                height={rectH + 4}
                rx={4}
                fill="none"
                stroke={config.color}
                strokeWidth={1.5}
                strokeDasharray="3 3"
                className="pointer-events-none"
              />
            )}
            {/* Label inside rect if wide enough (text is truncated by length check) */}
            {w > 40 && (
              <text
                x={x + w / 2}
                y={midY}
                textAnchor="middle"
                dominantBaseline="central"
                fontSize={9}
                fill={contrastLabelColor(config.color)}
                className="pointer-events-none select-none"
              >
                {f.description && f.description.length > w / 6
                  ? f.description.slice(0, Math.floor(w / 6)) + '…'
                  : f.description || f.feature_type}
              </text>
            )}
          </g>
        );
      })}
    </g>
  );
});
