/**
 * DiMA Desktop - Panel Toggle Sidebar
 * 
 * Drawer/sidebar for toggling panel visibility.
 */

import { Eye, EyeOff } from 'lucide-react';
import { cn } from '@/lib/utils';

interface PanelToggleProps {
  panels: { id: string; title: string }[];
  hiddenPanels: string[];
  onTogglePanel: (panelId: string) => void;
}

const PANEL_INFO = [
  { id: 'entropy-line', title: 'Entropy Chart' },
  { id: 'position-explorer', title: 'Position Explorer' },
  { id: 'variant-distribution', title: 'Motif Distribution' },
  { id: 'metadata-chart', title: 'Sequence Metadata' },
  { id: 'hcs-map', title: 'Highly Conserved Sequences' },
  { id: 'pdb-viewer', title: '3D Structure' },
];

export function PanelToggle({ hiddenPanels, onTogglePanel }: PanelToggleProps) {
  return (
    <div className="w-64 border-l bg-card p-4">
      <h3 className="mb-4 font-semibold">Panels</h3>
      <div className="space-y-2">
        {PANEL_INFO.map((panel) => {
          const isHidden = hiddenPanels.includes(panel.id);
          return (
            <button
              key={panel.id}
              onClick={() => onTogglePanel(panel.id)}
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
              <span className="text-sm">{panel.title}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
