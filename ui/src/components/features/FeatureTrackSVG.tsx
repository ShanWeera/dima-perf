/**
 * FeatureTrackSVG - SVG container for all feature category tracks.
 *
 * Renders an HCS background track, category tracks, and a position axis.
 * Pure presentational: receives all data via props, no store connections.
 */

import React, { useMemo, useState, useRef, useCallback, useEffect, useId } from 'react';
import type { MappedFeature } from '@/lib/types';
import type { HCSRegionSimple } from '@/lib/hcs';
import {
  FEATURE_CATEGORIES,
  FEATURE_CATEGORY_ORDER,
  groupFeaturesByCategory,
} from '@/lib/features';
import { FeatureTrack } from './FeatureTrack';
import { FeatureTooltipContent } from './FeatureTooltipContent';

interface FeatureTrackSVGProps {
  /** All mapped features (filtered to visible categories by caller) */
  features: MappedFeature[];
  /** Which categories to show */
  visibleCategories: Set<string>;
  /** Total MSA sequence length */
  sequenceLength: number;
  /** Container width in pixels */
  width: number;
  /** HCS regions for the background overlay */
  hcsRegions: HCSRegionSimple[];
  /** Hovered feature (for highlight) */
  hoveredFeature: MappedFeature | null;
  /** Selected feature (for ring) */
  selectedFeature: MappedFeature | null;
  /** Hover callback */
  onHover: (feature: MappedFeature) => void;
  /** Leave callback */
  onLeave: () => void;
  /** Click callback */
  onClick: (feature: MappedFeature) => void;
}

const TRACK_HEIGHT = 22;
const LABEL_WIDTH = 90;
const AXIS_HEIGHT = 20;
const HCS_TRACK_HEIGHT = 12;
const PADDING_TOP = 4;
const MINIMAP_HEIGHT = 14;
const MINIMAP_GAP = 4;

export const FeatureTrackSVG = React.memo(function FeatureTrackSVG({
  features,
  visibleCategories,
  sequenceLength,
  width,
  hcsRegions,
  hoveredFeature,
  selectedFeature,
  onHover,
  onLeave,
  onClick,
}: FeatureTrackSVGProps) {
  // Unique ID prefix for SVG clip paths, preventing ID collisions when
  // multiple FeatureTrackSVG instances coexist (e.g., normal + fullscreen). (Fix 4.15)
  const uniqueId = useId();
  const clipId = `drawAreaClip-${uniqueId}`;

  const [tooltipInfo, setTooltipInfo] = useState<{
    feature: MappedFeature;
    x: number;
    y: number;
  } | null>(null);
  const svgRef = useRef<SVGSVGElement>(null);

  // Viewport state for zoom/pan (as fractions of the sequence, 0-1)
  const [viewStart, setViewStart] = useState(0);
  const [viewEnd, setViewEnd] = useState(1);
  // Tracks whether a drag is in progress for cursor styling.
  // Separate from isDragging ref because state changes trigger re-render for the cursor class.
  const [isDraggingState, setIsDraggingState] = useState(false);
  const isDragging = useRef(false);
  const dragStartX = useRef(0);
  const dragViewStart = useRef(0);
  const dragViewEnd = useRef(0);
  // Tracks cumulative drag distance to suppress click events after a pan gesture.
  // Without this, every pan that starts on a feature would also select/deselect it.
  const dragDistanceRef = useRef(0);
  const didDragRef = useRef(false);

  // Wrap the parent onHover to also capture mouse position for tooltip
  const handleHover = useCallback(
    (feature: MappedFeature) => {
      onHover(feature);
    },
    [onHover]
  );

  const handleLeave = useCallback(() => {
    onLeave();
    setTooltipInfo(null);
  }, [onLeave]);

  // Track mouse position on the SVG for tooltip placement
  const handleMouseMove = useCallback(
    (e: React.MouseEvent<SVGSVGElement>) => {
      if (hoveredFeature && svgRef.current) {
        const rect = svgRef.current.getBoundingClientRect();
        setTooltipInfo({
          feature: hoveredFeature,
          x: e.clientX - rect.left,
          y: e.clientY - rect.top,
        });
      }
    },
    [hoveredFeature]
  );

  // Zoom via mouse wheel, pan via shift+wheel or drag.
  // IMPORTANT: We attach the wheel handler natively with { passive: false } because
  // React registers wheel listeners as passive by default, which prevents
  // e.preventDefault() from working (throws in Chrome/Edge).
  const wheelHandlerRef = useRef<(e: WheelEvent) => void>();
  wheelHandlerRef.current = (e: WheelEvent) => {
    e.preventDefault();
    const viewSpan = viewEnd - viewStart;

    if (e.shiftKey) {
      // Pan: shift + scroll horizontally
      const panAmount = (e.deltaY / width) * viewSpan * 2;
      const newStart = Math.max(0, Math.min(1 - viewSpan, viewStart + panAmount));
      setViewStart(newStart);
      setViewEnd(newStart + viewSpan);
    } else {
      // Zoom: scroll vertically to zoom in/out around the cursor position
      const zoomFactor = e.deltaY > 0 ? 1.15 : 0.87;
      const svgRect = svgRef.current?.getBoundingClientRect();
      // Cursor position as fraction of the draw area
      const cursorFrac = svgRect
        ? Math.max(0, Math.min(1, (e.clientX - svgRect.left - LABEL_WIDTH) / (svgRect.width - LABEL_WIDTH)))
        : 0.5;
      // Position in sequence space that cursor points to
      const cursorPos = viewStart + cursorFrac * viewSpan;

      const newSpan = Math.min(1, Math.max(0.01, viewSpan * zoomFactor));
      let newStart = cursorPos - cursorFrac * newSpan;
      let newEnd = cursorPos + (1 - cursorFrac) * newSpan;

      // Clamp to [0, 1]
      if (newStart < 0) { newEnd -= newStart; newStart = 0; }
      if (newEnd > 1) { newStart -= (newEnd - 1); newEnd = 1; }
      newStart = Math.max(0, newStart);
      newEnd = Math.min(1, newEnd);

      setViewStart(newStart);
      setViewEnd(newEnd);
    }
  };

  // Attach the wheel handler natively so we can use { passive: false }
  useEffect(() => {
    const el = svgRef.current;
    if (!el) return;
    const handler = (e: WheelEvent) => wheelHandlerRef.current?.(e);
    el.addEventListener('wheel', handler, { passive: false });
    return () => el.removeEventListener('wheel', handler);
  }, []);

  // Drag to pan
  const handleMouseDown = useCallback(
    (e: React.MouseEvent<SVGSVGElement>) => {
      // Only start drag on left click
      if (e.button !== 0) return;
      isDragging.current = true;
      setIsDraggingState(true);
      dragStartX.current = e.clientX;
      dragViewStart.current = viewStart;
      dragViewEnd.current = viewEnd;
      // Reset drag distance for click suppression tracking
      dragDistanceRef.current = 0;
      didDragRef.current = false;
    },
    [viewStart, viewEnd]
  );

  useEffect(() => {
    const handleGlobalMouseMove = (e: MouseEvent) => {
      if (!isDragging.current || !svgRef.current) return;
      const drawW = svgRef.current.getBoundingClientRect().width - LABEL_WIDTH;
      const dx = e.clientX - dragStartX.current;
      // Track cumulative distance so we can suppress click after significant drag
      dragDistanceRef.current = Math.abs(dx);
      const viewSpan = dragViewEnd.current - dragViewStart.current;
      const panAmount = -(dx / drawW) * viewSpan;

      let newStart = dragViewStart.current + panAmount;
      let newEnd = dragViewEnd.current + panAmount;
      if (newStart < 0) { newEnd -= newStart; newStart = 0; }
      if (newEnd > 1) { newStart -= (newEnd - 1); newEnd = 1; }
      setViewStart(Math.max(0, newStart));
      setViewEnd(Math.min(1, newEnd));
    };

    const handleGlobalMouseUp = () => {
      // Mark as "was dragging" if moved more than 3px, so we can suppress
      // the click event that fires immediately after mouse up on feature shapes.
      if (isDragging.current && dragDistanceRef.current > 3) {
        didDragRef.current = true;
        // Reset the flag after a tick so only the immediately following click is suppressed
        requestAnimationFrame(() => { didDragRef.current = false; });
      }
      isDragging.current = false;
      setIsDraggingState(false);
    };

    window.addEventListener('mousemove', handleGlobalMouseMove);
    window.addEventListener('mouseup', handleGlobalMouseUp);
    return () => {
      window.removeEventListener('mousemove', handleGlobalMouseMove);
      window.removeEventListener('mouseup', handleGlobalMouseUp);
    };
  }, []);

  // Wrap click to suppress it when a drag gesture just ended.
  // Without this, releasing a pan on a feature shape fires both mouseUp (ending pan)
  // AND onClick (toggling feature selection).
  const handleClickGuarded = useCallback(
    (feature: MappedFeature) => {
      if (didDragRef.current) return; // suppress click after drag
      onClick(feature);
    },
    [onClick]
  );

  // Double-click to reset zoom
  const handleDoubleClick = useCallback(() => {
    setViewStart(0);
    setViewEnd(1);
  }, []);

  // Group features by category (memoized)
  const grouped = useMemo(() => groupFeaturesByCategory(features), [features]);

  // Ordered list of visible categories that have features
  const visibleTracks = useMemo(
    () =>
      FEATURE_CATEGORY_ORDER.filter(
        (key) => visibleCategories.has(key) && (grouped.get(key)?.length ?? 0) > 0
      ),
    [visibleCategories, grouped]
  );

  const drawWidth = width - LABEL_WIDTH;
  // Total height includes minimap at the top
  const svgHeight =
    MINIMAP_HEIGHT + MINIMAP_GAP +
    PADDING_TOP +
    visibleTracks.length * TRACK_HEIGHT +
    HCS_TRACK_HEIGHT +
    AXIS_HEIGHT;

  // The vertical offset where tracks start (after minimap)
  const tracksTopY = MINIMAP_HEIGHT + MINIMAP_GAP;

  // Viewport-aware position → pixel mapping.
  // Maps sequence positions in [viewStart * seqLen, viewEnd * seqLen] to the draw area.
  const viewSeqStart = viewStart * sequenceLength;
  const viewSeqEnd = viewEnd * sequenceLength;
  // Guard against zero span (when viewStart === viewEnd) to prevent
  // division by zero in posToX. (Fix 4.15)
  const viewSeqSpan = Math.max(viewSeqEnd - viewSeqStart, 1);
  const posToX = (pos: number) =>
    LABEL_WIDTH + ((pos - viewSeqStart) / viewSeqSpan) * drawWidth;

  // Full-sequence position → pixel for the minimap (always shows full protein)
  const minimapPosToX = (pos: number) =>
    LABEL_WIDTH + (pos / sequenceLength) * drawWidth;

  if (sequenceLength === 0 || width <= LABEL_WIDTH) {
    return (
      <div className="flex h-full items-center justify-center text-xs text-muted-foreground">
        {sequenceLength === 0 ? 'No sequence data' : 'Panel too narrow for feature track'}
      </div>
    );
  }

  return (
    <div className="relative">
    <svg
      ref={svgRef}
      width={width}
      height={svgHeight}
      className={`select-none ${isDraggingState ? 'cursor-grabbing' : 'cursor-default'}`}
      role="img"
      aria-label="Protein feature tracks aligned to MSA positions"
      onMouseMove={handleMouseMove}
      onMouseLeave={() => { setTooltipInfo(null); onLeave(); }}
      onMouseDown={handleMouseDown}
      onDoubleClick={handleDoubleClick}
    >
      {/* Clip path to prevent features from rendering outside the draw area
          when zoomed in (features with positions before viewStart would have
          negative x values and spill into the label column). */}
      <defs>
        <clipPath id={clipId}>
          <rect x={LABEL_WIDTH} y={0} width={drawWidth} height={svgHeight} />
        </clipPath>
      </defs>

      {/* Minimap: always shows the full protein with viewport indicator */}
      <g>
        {/* Minimap background */}
        <rect
          x={LABEL_WIDTH}
          y={1}
          width={drawWidth}
          height={MINIMAP_HEIGHT - 2}
          rx={2}
          fill="currentColor"
          fillOpacity={0.03}
          stroke="currentColor"
          strokeOpacity={0.1}
          strokeWidth={0.5}
        />
        {/* Minimap: HCS regions as tiny green bars */}
        {hcsRegions.map((hcs, i) => {
          const mx = minimapPosToX(hcs.startPosition);
          const mw = Math.max(minimapPosToX(hcs.endPosition) - mx, 1);
          return (
            <rect
              key={`mm-hcs-${i}`}
              x={mx}
              y={3}
              width={mw}
              height={MINIMAP_HEIGHT - 6}
              rx={1}
              className="fill-green-500/30 dark:fill-green-400/30"
            />
          );
        })}
        {/* Minimap: feature category marks */}
        {features.map((f, i) => {
          if (f.msaBegin === null || f.msaEnd === null) return null;
          if (!visibleCategories.has(f.categoryKey)) return null;
          const config = FEATURE_CATEGORIES[f.categoryKey];
          const mx = minimapPosToX(f.msaBegin);
          const mw = Math.max(minimapPosToX(f.msaEnd + 1) - mx, 1);
          return (
            <rect
              key={`mm-f-${i}`}
              x={mx}
              y={4}
              width={mw}
              height={MINIMAP_HEIGHT - 8}
              fill={config?.color ?? '#888'}
              fillOpacity={0.4}
            />
          );
        })}
        {/* Viewport indicator: highlighted rectangle showing current view */}
        <rect
          x={LABEL_WIDTH + viewStart * drawWidth}
          y={1}
          width={Math.max((viewEnd - viewStart) * drawWidth, 4)}
          height={MINIMAP_HEIGHT - 2}
          rx={2}
          fill="currentColor"
          fillOpacity={0.08}
          stroke="currentColor"
          strokeOpacity={0.25}
          strokeWidth={1}
          className="pointer-events-none"
        />
        {/* Minimap label */}
        <text
          x={LABEL_WIDTH - 6}
          y={MINIMAP_HEIGHT / 2}
          textAnchor="end"
          dominantBaseline="central"
          className="fill-muted-foreground"
          fontSize={8}
        >
          Overview
        </text>
      </g>

      {/* HCS background track (bottom, subtle) — offset by tracksTopY, clipped to draw area */}
      <g transform={`translate(0, ${tracksTopY})`} clipPath={`url(#${clipId})`}>
        <text
          x={LABEL_WIDTH - 6}
          y={PADDING_TOP + visibleTracks.length * TRACK_HEIGHT + HCS_TRACK_HEIGHT / 2}
          textAnchor="end"
          dominantBaseline="central"
          className="fill-green-600 dark:fill-green-400"
          fontSize={9}
          fontWeight={500}
        >
          HCS
        </text>
        {hcsRegions.map((hcs, i) => {
          const x = posToX(hcs.startPosition);
          // +1 for inclusive end position (both start and end are part of the region)
          const w = Math.max(posToX(hcs.endPosition + 1) - x, 2);
          const y = PADDING_TOP + visibleTracks.length * TRACK_HEIGHT;
          return (
            <rect
              key={`hcs-${i}`}
              x={x}
              y={y + 1}
              width={w}
              height={HCS_TRACK_HEIGHT - 2}
              rx={2}
              className="fill-green-500/15 stroke-green-500/30 dark:fill-green-400/15 dark:stroke-green-400/30"
              strokeWidth={0.5}
            />
          );
        })}
      </g>

      {/* Feature category tracks — offset by tracksTopY, clipped to draw area */}
      <g transform={`translate(0, ${tracksTopY})`} clipPath={`url(#${clipId})`}>
        {visibleTracks.map((key, idx) => {
          const config = FEATURE_CATEGORIES[key];
          const trackFeatures = grouped.get(key) ?? [];
          return (
            <FeatureTrack
              key={key}
              category={key}
              config={config}
              features={trackFeatures}
              viewSeqStart={viewSeqStart}
              viewSeqSpan={viewSeqSpan}
              svgWidth={width}
              yOffset={PADDING_TOP + idx * TRACK_HEIGHT}
              trackHeight={TRACK_HEIGHT}
              labelWidth={LABEL_WIDTH}
              hoveredFeature={hoveredFeature}
              selectedFeature={selectedFeature}
              onMouseEnter={handleHover}
              onMouseLeave={handleLeave}
              onClick={handleClickGuarded}
            />
          );
        })}
      </g>

      {/* Position axis — offset by tracksTopY */}
      <g transform={`translate(0, ${tracksTopY + PADDING_TOP + visibleTracks.length * TRACK_HEIGHT + HCS_TRACK_HEIGHT})`}>
        <line
          x1={LABEL_WIDTH}
          y1={0}
          x2={width}
          y2={0}
          stroke="currentColor"
          strokeOpacity={0.15}
        />
        {/* Tick marks at regular intervals within the visible viewport */}
        {Array.from({ length: 6 }, (_, i) => {
          const pos = Math.round(viewSeqStart + (viewSeqSpan * i) / 5);
          const x = posToX(pos);
          return (
            <g key={i}>
              <line x1={x} y1={0} x2={x} y2={4} stroke="currentColor" strokeOpacity={0.3} />
              <text
                x={x}
                y={14}
                textAnchor="middle"
                className="fill-muted-foreground"
                fontSize={9}
              >
                {pos}
              </text>
            </g>
          );
        })}
      </g>
    </svg>

    {/* Rich tooltip overlay (positioned above the SVG based on mouse position) */}
    {tooltipInfo && (
      <div
        className="absolute z-50 pointer-events-none rounded-md border bg-popover p-2 shadow-md animate-in fade-in-0 zoom-in-95"
        style={{
          left: Math.min(tooltipInfo.x + 12, width - 260),
          top: Math.max(tooltipInfo.y - 10, 0),
          maxWidth: 260,
        }}
      >
        <FeatureTooltipContent feature={tooltipInfo.feature} />
      </div>
    )}
    </div>
  );
});
