/**
 * DiMA Desktop - Toast Notification Store
 *
 * Minimal toast notification system using Zustand.
 * Provides success/error/info feedback without external dependencies.
 *
 * Features:
 * - Maximum visible toast cap to prevent unbounded accumulation
 * - Timer tracking so manual dismiss cancels the auto-dismiss timer
 * - Message deduplication within a short window to avoid spam
 */

import { create } from 'zustand';

export type ToastVariant = 'success' | 'error' | 'info' | 'warning';

export interface ToastAction {
  label: string;
  onClick: () => void;
}

export interface Toast {
  id: string;
  message: string;
  variant: ToastVariant;
  /** Auto-dismiss delay in ms. Set to 0 for persistent toasts. */
  duration: number;
  /** Optional action button shown alongside the toast message. */
  action?: ToastAction;
}

interface ToastState {
  toasts: Toast[];
  addToast: (message: string, variant?: ToastVariant, duration?: number, action?: ToastAction) => void;
  removeToast: (id: string) => void;
}

const MAX_VISIBLE_TOASTS = 5;
const DEDUP_WINDOW_MS = 2000;

const DEFAULT_DURATION_MS: Record<ToastVariant, number> = {
  success: 3000,
  info: 4000,
  warning: 5000,
  error: 6000,
};

// Track active timers so we can cancel on manual dismiss
const timerMap = new Map<string, ReturnType<typeof setTimeout>>();

// Track recent messages for deduplication
const recentMessages = new Map<string, number>();

export const useToastStore = create<ToastState>((set) => ({
  toasts: [],

  addToast: (message, variant = 'info', duration?, action?) => {
    const dedupKey = `${variant}:${message}`;
    const now = Date.now();

    // Prune expired entries to prevent unbounded growth in long sessions (Fix 5.107).
    // Two-tier strategy: always prune stale entries, then hard-cap at 100 entries
    // by removing oldest timestamps if many unique messages arrive rapidly.
    if (recentMessages.size > 30) {
      for (const [key, ts] of recentMessages) {
        if (now - ts > DEDUP_WINDOW_MS) recentMessages.delete(key);
      }
      // Hard cap as final safety net
      if (recentMessages.size > 100) {
        const entries = [...recentMessages.entries()].sort((a, b) => a[1] - b[1]);
        const toRemove = entries.slice(0, entries.length - 50);
        for (const [key] of toRemove) recentMessages.delete(key);
      }
    }

    // Deduplicate: skip if the same message+variant was shown within DEDUP_WINDOW_MS
    const lastShown = recentMessages.get(dedupKey);
    if (lastShown && now - lastShown < DEDUP_WINDOW_MS) {
      return;
    }
    recentMessages.set(dedupKey, now);

    const id = crypto.randomUUID();
    const toast: Toast = {
      id,
      message,
      variant,
      duration: duration ?? DEFAULT_DURATION_MS[variant],
      action,
    };

    set((state) => {
      const updated = [...state.toasts, toast];
      // Evict oldest toasts beyond the cap
      if (updated.length > MAX_VISIBLE_TOASTS) {
        const evicted = updated.slice(0, updated.length - MAX_VISIBLE_TOASTS);
        for (const t of evicted) {
          const timer = timerMap.get(t.id);
          if (timer) { clearTimeout(timer); timerMap.delete(t.id); }
        }
        return { toasts: updated.slice(-MAX_VISIBLE_TOASTS) };
      }
      return { toasts: updated };
    });

    if (toast.duration > 0) {
      const timer = setTimeout(() => {
        timerMap.delete(id);
        set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) }));
      }, toast.duration);
      timerMap.set(id, timer);
    }
  },

  removeToast: (id) => {
    // Cancel the auto-dismiss timer on manual dismiss
    const timer = timerMap.get(id);
    if (timer) { clearTimeout(timer); timerMap.delete(id); }
    set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) }));
  },
}));
