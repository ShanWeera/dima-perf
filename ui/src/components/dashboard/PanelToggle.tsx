/**
 * DiMA Desktop - Panel Toggle Sidebar
 * 
 * Drawer/sidebar for toggling panel visibility and resetting layout.
 */

import { Eye, EyeOff, RotateCcw } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import { PANEL_INFO } from '@/lib/dashboard-layout';

interface PanelToggleProps {
  hiddenPanels: string[];
  onTogglePanel: (panelId: string) => void;
  onResetLayout?: () => void;
}

export function PanelToggle({ hiddenPanels, onTogglePanel, onResetLayout }: PanelToggleProps) {
  return (
    <div className="w-64 border-l bg-card p-4">
      <div className="flex items-center justify-between mb-4">
        <h3 className="font-semibold">Panels</h3>
        {onResetLayout && (
          <Button
            variant="ghost"
            size="sm"
            className="h-7 px-2 text-xs gap-1"
            onClick={onResetLayout}
            aria-label="Reset dashboard layout to default"
          >
            <RotateCcw className="h-3 w-3" />
            Reset
          </Button>
        )}
      </div>
      <div className="space-y-2">
        {PANEL_INFO.map((panel) => {
          const isHidden = hiddenPanels.includes(panel.id);
          return (
            <button
              key={panel.id}
              onClick={() => onTogglePanel(panel.id)}
              aria-pressed={!isHidden}
              aria-label={`${isHidden ? 'Show' : 'Hide'} ${panel.label} panel`}
              className={cn(
                "flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left transition-colors",
                isHidden ? "opacity-50 hover:bg-muted" : "bg-muted/50 hover:bg-muted"
              )}
            >
              {isHidden ? (
                <EyeOff className="h-4 w-4 text-muted-foreground" />
              ) : (
                <Eye className="h-4 w-4 text-primary" />
              )}
              <span className="text-sm">{panel.label}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
