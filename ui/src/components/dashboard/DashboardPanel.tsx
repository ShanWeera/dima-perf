/**
 * DiMA Desktop - Dashboard Panel
 * 
 * Wrapper component for dashboard panels with header and controls.
 */

import { useState, useEffect, useRef, memo } from 'react';
import { createPortal } from 'react-dom';
import { GripVertical, Maximize2, X, Download } from 'lucide-react';
import { cn } from '@/lib/utils';

interface DashboardPanelProps {
  title: string;
  subtitle?: string;
  panelId: string;
  children: React.ReactNode;
  className?: string;
  onExportChart?: () => void;
}

export const DashboardPanel = memo(function DashboardPanel({
  title,
  subtitle,
  panelId: _panelId,
  children,
  className,
  onExportChart,
}: DashboardPanelProps) {
  const [isFullscreen, setIsFullscreen] = useState(false);
  const fullscreenRef = useRef<HTMLDivElement>(null);

  // Close fullscreen on Escape, trap focus, and lock body scroll
  useEffect(() => {
    if (!isFullscreen) return;
    const container = fullscreenRef.current;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setIsFullscreen(false);
        return;
      }
      // Focus trap: cycle Tab within the fullscreen overlay
      if (e.key === 'Tab' && container) {
        const focusable = container.querySelectorAll<HTMLElement>(
          'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
        );
        if (focusable.length === 0) return;
        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    };

    // Lock body scroll while fullscreen is open
    const prevOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';

    document.addEventListener('keydown', handleKeyDown);
    container?.focus();

    return () => {
      document.removeEventListener('keydown', handleKeyDown);
      document.body.style.overflow = prevOverflow;
    };
  }, [isFullscreen]);

  // Portal to document.body so the fullscreen overlay escapes react-grid-layout's
  // CSS transform on grid items, which would otherwise trap position:fixed within the cell.
  if (isFullscreen) {
    return createPortal(
      <>
      {/* Dimmed backdrop */}
      <div className="fixed inset-0 z-40 bg-black/50" aria-hidden="true" />
      <div
        ref={fullscreenRef}
        className="fixed inset-0 z-50 flex flex-col bg-background"
        role="dialog"
        aria-modal="true"
        aria-label={`${title} (fullscreen)`}
        tabIndex={-1}
      >
        {/* Fullscreen Header */}
        <div className="flex items-center justify-between border-b px-4 py-3">
          <div className="min-w-0 flex-1 mr-2">
            <h3 className="truncate font-semibold">{title}</h3>
            {subtitle && (
              <p className="truncate text-sm text-muted-foreground" title={subtitle}>{subtitle}</p>
            )}
          </div>
          <button
            onClick={() => setIsFullscreen(false)}
            className="rounded-md p-2 hover:bg-muted"
            aria-label="Exit fullscreen"
          >
            <X className="h-5 w-5" />
          </button>
        </div>
        {/* Fullscreen Content */}
        <div className="flex-1 overflow-hidden p-4">
          {children}
        </div>
      </div>
      </>,
      document.body
    );
  }

  return (
    <div className={cn(
      "flex h-full flex-col overflow-hidden rounded-lg border bg-card shadow-sm",
      className
    )}>
      {/* Panel Header */}
      <div className="panel-handle flex cursor-move items-center justify-between border-b bg-muted/50 px-3 py-2 shrink-0">
        <div className="flex items-center gap-2 min-w-0">
          <GripVertical className="h-4 w-4 text-muted-foreground shrink-0" />
          <div className="min-w-0">
            <h3 className="text-sm font-medium truncate">{title}</h3>
            {subtitle && (
              <p className="text-xs text-muted-foreground truncate">{subtitle}</p>
            )}
          </div>
        </div>
        <div className="flex items-center gap-1 shrink-0">
          {onExportChart && (
            <button
              onClick={onExportChart}
              onMouseDown={(e) => e.stopPropagation()}
              className="rounded p-1 hover:bg-muted"
              aria-label={`Export ${title} as image`}
              title="Export chart"
            >
              <Download className="h-3.5 w-3.5 text-muted-foreground" />
            </button>
          )}
          <button
            onClick={() => setIsFullscreen(true)}
            onMouseDown={(e) => e.stopPropagation()}
            className="rounded p-1 hover:bg-muted"
            aria-label={`Fullscreen ${title}`}
          >
            <Maximize2 className="h-4 w-4 text-muted-foreground" />
          </button>
        </div>
      </div>
      {/* Panel Content */}
      <div className="flex-1 overflow-auto min-h-0 min-w-0">
        {children}
      </div>
    </div>
  );
});
