/**
 * DiMA Desktop - Toast Container
 * 
 * Renders active toast notifications as a stacked overlay in the bottom-right corner.
 */

import { useToastStore, type ToastVariant } from '@/stores/toastStore';
import { X, CheckCircle2, AlertCircle, Info, AlertTriangle } from 'lucide-react';
import { cn } from '@/lib/utils';

const ICON_MAP: Record<ToastVariant, typeof CheckCircle2> = {
  success: CheckCircle2,
  error: AlertCircle,
  info: Info,
  warning: AlertTriangle,
};

const STYLE_MAP: Record<ToastVariant, string> = {
  success: 'border-green-500/30 bg-green-50 text-green-900 dark:bg-green-950/50 dark:text-green-100',
  error: 'border-red-500/30 bg-red-50 text-red-900 dark:bg-red-950/50 dark:text-red-100',
  info: 'border-blue-500/30 bg-blue-50 text-blue-900 dark:bg-blue-950/50 dark:text-blue-100',
  warning: 'border-yellow-500/30 bg-yellow-50 text-yellow-900 dark:bg-yellow-950/50 dark:text-yellow-100',
};

const ICON_STYLE_MAP: Record<ToastVariant, string> = {
  success: 'text-green-600 dark:text-green-400',
  error: 'text-red-600 dark:text-red-400',
  info: 'text-blue-600 dark:text-blue-400',
  warning: 'text-yellow-600 dark:text-yellow-400',
};

export function ToastContainer() {
  const { toasts, removeToast } = useToastStore();

  if (toasts.length === 0) return null;

  return (
    <div className="fixed bottom-4 right-4 z-[60] flex flex-col gap-2 max-w-sm">
      {toasts.map((toast) => {
        const Icon = ICON_MAP[toast.variant];
        return (
          <div
            key={toast.id}
            className={cn(
              'flex items-start gap-2 rounded-lg border px-4 py-3 shadow-lg animate-in slide-in-from-right-full fade-in duration-200',
              STYLE_MAP[toast.variant]
            )}
            role={toast.variant === 'error' || toast.variant === 'warning' ? 'alert' : 'status'}
            aria-live={toast.variant === 'error' || toast.variant === 'warning' ? 'assertive' : 'polite'}
          >
            <Icon className={cn('h-5 w-5 shrink-0 mt-0.5', ICON_STYLE_MAP[toast.variant])} />
            <div className="flex-1 min-w-0">
              <p className="text-sm break-words">{toast.message}</p>
              {toast.action && (
                <button
                  onClick={() => {
                    toast.action!.onClick();
                    removeToast(toast.id);
                  }}
                  className="mt-1 text-xs font-medium underline underline-offset-2 hover:opacity-80 transition-opacity"
                >
                  {toast.action.label}
                </button>
              )}
            </div>
            <button
              onClick={() => removeToast(toast.id)}
              className="shrink-0 rounded p-0.5 opacity-60 hover:opacity-100 transition-opacity"
              aria-label="Dismiss"
            >
              <X className="h-4 w-4" />
            </button>
          </div>
        );
      })}
    </div>
  );
}
