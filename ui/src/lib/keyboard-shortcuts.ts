/**
 * Keyboard Shortcuts — constants, registration utility, and conflict guards.
 *
 * Handles cross-platform Mod key (Cmd on macOS, Ctrl on Win/Linux),
 * prevents firing inside interactive elements, and provides cleanup
 * functions for React useEffect teardown.
 */

const isMac = typeof navigator !== 'undefined' && /Mac|iPhone|iPad|iPod/.test(navigator.platform);

export interface ShortcutDef {
  /** Human-readable key combo for display */
  display: string;
  /** Description of what this shortcut does */
  description: string;
  /** The keyboard key (e.g. 'e', ',', '?', 'Escape') */
  key: string;
  /** Whether Mod (Cmd/Ctrl) is required */
  mod?: boolean;
  /** Whether Shift is required */
  shift?: boolean;
}

/** All registered shortcut definitions for the help dialog */
export const SHORTCUTS: Record<string, ShortcutDef> = {
  export: {
    display: isMac ? '⌘⇧E' : 'Ctrl+Shift+E',
    description: 'Export results',
    key: 'e',
    mod: true,
    shift: true,
  },
  toggleFilters: {
    display: isMac ? '⌘⇧F' : 'Ctrl+Shift+F',
    description: 'Toggle filters panel',
    key: 'f',
    mod: true,
    shift: true,
  },
  newAnalysis: {
    display: isMac ? '⌘⇧A' : 'Ctrl+Shift+A',
    description: 'New analysis',
    key: 'a',
    mod: true,
    shift: true,
  },
  settings: {
    display: isMac ? '⌘,' : 'Ctrl+,',
    description: 'Open settings',
    key: ',',
    mod: true,
  },
  escape: {
    display: 'Esc',
    description: 'Close panel / dialog',
    key: 'Escape',
  },
  help: {
    display: '?',
    description: 'Show keyboard shortcuts',
    key: '?',
  },
};

/**
 * Checks whether the event target is inside an interactive element
 * where keyboard shortcuts should NOT fire (inputs, textareas, dialogs, etc.).
 */
function shouldIgnoreEvent(e: KeyboardEvent): boolean {
  const target = e.target as HTMLElement | null;
  if (!target) return false;

  const tag = target.tagName;
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return true;
  if (target.isContentEditable) return true;

  // Inside a Radix dialog or portal — let Radix handle its own keyboard events
  if (target.closest('[role="dialog"]') || target.closest('[role="alertdialog"]')) return true;

  return false;
}

/**
 * Checks whether the Mod key is pressed (Cmd on Mac, Ctrl on Win/Linux).
 */
function isModPressed(e: KeyboardEvent): boolean {
  return isMac ? e.metaKey : e.ctrlKey;
}

export interface ShortcutHandler {
  shortcut: ShortcutDef;
  handler: () => void;
}

/**
 * Registers a set of keyboard shortcuts on the `window` keydown event.
 * Returns a cleanup function for use in useEffect teardown.
 *
 * Event propagation note: DashboardPanel registers its Escape listener on
 * `document` (fires before `window` in bubble order) and stops propagation,
 * so the global Escape handler won't conflict with fullscreen panel close.
 */
export function registerShortcuts(shortcuts: ShortcutHandler[]): () => void {
  const handleKeyDown = (e: KeyboardEvent) => {
    if (shouldIgnoreEvent(e)) return;

    for (const { shortcut, handler } of shortcuts) {
      const modMatch = shortcut.mod ? isModPressed(e) : !isModPressed(e);
      const shiftMatch = shortcut.shift ? e.shiftKey : !e.shiftKey;
      const keyMatch = e.key.toLowerCase() === shortcut.key.toLowerCase();

      if (modMatch && shiftMatch && keyMatch) {
        e.preventDefault();
        handler();
        return;
      }
    }
  };

  window.addEventListener('keydown', handleKeyDown);
  return () => window.removeEventListener('keydown', handleKeyDown);
}
