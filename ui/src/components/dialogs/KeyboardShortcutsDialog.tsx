/**
 * Keyboard Shortcuts Dialog — displays all available keyboard shortcuts
 * in a grid layout. Triggered by pressing '?' anywhere in the app.
 */

import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { SHORTCUTS } from '@/lib/keyboard-shortcuts';

interface KeyboardShortcutsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function KeyboardShortcutsDialog({ open, onOpenChange }: KeyboardShortcutsDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Keyboard Shortcuts</DialogTitle>
        </DialogHeader>
        <div className="grid gap-2 py-2">
          {Object.values(SHORTCUTS).map((shortcut) => (
            <div
              key={shortcut.key + (shortcut.mod ? 'mod' : '') + (shortcut.shift ? 'shift' : '')}
              className="flex items-center justify-between rounded-md px-3 py-2 hover:bg-muted/50"
            >
              <span className="text-sm text-foreground">{shortcut.description}</span>
              <kbd className="inline-flex items-center rounded border bg-muted px-2 py-0.5 text-xs font-mono text-muted-foreground">
                {shortcut.display}
              </kbd>
            </div>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  );
}
