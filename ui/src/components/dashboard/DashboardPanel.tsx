/**
 * DiMA Desktop - Dashboard Panel
 * 
 * Wrapper component for dashboard panels with header and controls.
 */

import { useState } from 'react';
import { GripVertical, Maximize2, X } from 'lucide-react';
import { cn } from '@/lib/utils';

interface DashboardPanelProps {
  title: string;
  subtitle?: string;
  panelId: string;
  children: React.ReactNode;
  className?: string;
}

export function DashboardPanel({
  title,
  subtitle,
  panelId: _panelId,
  children,
  className,
}: DashboardPanelProps) {
  const [isFullscreen, setIsFullscreen] = useState(false);

  if (isFullscreen) {
    return (
      <div className="fixed inset-0 z-50 flex flex-col bg-background">
        {/* Fullscreen Header */}
        <div className="flex items-center justify-between border-b px-4 py-3">
          <div>
            <h3 className="font-semibold">{title}</h3>
            {subtitle && (
              <p className="text-sm text-muted-foreground">{subtitle}</p>
            )}
          </div>
          <button
            onClick={() => setIsFullscreen(false)}
            className="rounded-md p-2 hover:bg-muted"
          >
            <X className="h-5 w-5" />
          </button>
        </div>
        {/* Fullscreen Content */}
        <div className="flex-1 overflow-hidden p-4">
          {children}
        </div>
      </div>
    );
  }

  return (
    <div className={cn(
      "flex h-full flex-col overflow-visible rounded-lg border bg-card shadow-sm",
      className
    )}>
      {/* Panel Header */}
      <div className="panel-handle flex cursor-move items-center justify-between border-b bg-muted/50 px-3 py-2">
        <div className="flex items-center gap-2">
          <GripVertical className="h-4 w-4 text-muted-foreground shrink-0" />
          <div className="min-w-0">
            <h3 className="text-sm font-medium">{title}</h3>
            {subtitle && (
              <p className="text-xs text-muted-foreground truncate">{subtitle}</p>
            )}
          </div>
        </div>
        <button
          onClick={() => setIsFullscreen(true)}
          className="rounded p-1 hover:bg-muted shrink-0"
        >
          <Maximize2 className="h-4 w-4 text-muted-foreground" />
        </button>
      </div>
      {/* Panel Content */}
      <div className="flex-1 overflow-visible min-h-0 min-w-0">
        {children}
      </div>
    </div>
  );
}
