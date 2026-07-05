/**
 * DiMA Desktop - Utility Functions
 */

import { type ClassValue, clsx } from 'clsx';
import { twMerge } from 'tailwind-merge';
import { useToastStore } from '@/stores/toastStore';

/**
 * Merge Tailwind CSS classes with clsx
 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/**
 * Format bytes to human-readable size.
 * Handles values up to PB scale and guards against non-positive inputs.
 */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
  const i = Math.min(
    Math.floor(Math.log(bytes) / Math.log(k)),
    sizes.length - 1
  );
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

/**
 * Format a date string to a locale-aware human-readable format.
 * Returns an em-dash for invalid date strings.
 */
export function formatDate(dateString: string): string {
  const date = new Date(dateString);
  if (isNaN(date.getTime())) return '—';
  return date.toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}


/** Max project name length, consistent across all creation UI surfaces */
export const MAX_PROJECT_NAME_LENGTH = 100;

/** Characters invalid in file/folder names across OS platforms */
const INVALID_NAME_CHARS = /[<>:"/\\|?*]/;

/**
 * Validate a project name for creation/rename. (Fix 6.2)
 * Shared utility used by Sidebar, WelcomeView, ProjectsView, ImportDimaDialog.
 */
export function validateProjectName(name: string): { valid: boolean; error?: string } {
  const trimmed = name.trim();
  if (trimmed.length === 0) {
    return { valid: false, error: 'Project name cannot be empty' };
  }
  if (trimmed.length > MAX_PROJECT_NAME_LENGTH) {
    return { valid: false, error: `Project name must be ${MAX_PROJECT_NAME_LENGTH} characters or fewer` };
  }
  if (INVALID_NAME_CHARS.test(trimmed)) {
    return { valid: false, error: 'Project name contains invalid characters: < > : " / \\ | ? *' };
  }
  return { valid: true };
}

/**
 * Extract a human-readable message from a Tauri IPC error or JS Error.
 * Backend AppError serializes as `{ type: "ValidationError", message: "..." }`.
 * Tauri 2 may also wrap it in a string or nested object depending on the version.
 *
 * Handles these known shapes:
 * - Error instance: `.message`
 * - Plain string: returned directly
 * - `{ message: "..." }` — standard Error-like or AppError
 * - `{ type: "...", message: "..." }` — tagged AppError enum
 * - `{ data: { message: "..." } }` — Tauri v1 wrapping
 * - `{ error: "..." }` or `{ error: { message: "..." } }` — alternative shapes
 * - Last resort: JSON.stringify the object so the user sees *something* useful
 */
export function extractErrorMessage(error: unknown): string | null {
  if (error == null) return null;
  if (error instanceof Error) return error.message;
  if (typeof error === 'string') return error.length > 0 ? error : null;
  if (typeof error === 'number') return String(error);
  if (error && typeof error === 'object') {
    const obj = error as Record<string, unknown>;
    // Structured AppError from backend: { type: "...", message: "..." }
    if (typeof obj.message === 'string' && obj.message.length > 0) return obj.message;
    // Alternative key: { error: "..." }
    if (typeof obj.error === 'string' && obj.error.length > 0) return obj.error;
    // Nested: { error: { message: "..." } }
    if (obj.error && typeof obj.error === 'object') {
      const nested = obj.error as Record<string, unknown>;
      if (typeof nested.message === 'string') return nested.message;
    }
    // Tauri v1 style: { data: { message: "..." } }
    if (obj.data && typeof obj.data === 'object') {
      const data = obj.data as Record<string, unknown>;
      if (typeof data.message === 'string') return data.message;
    }
    // Last resort: JSON.stringify so the user never sees "[object Object]"
    try {
      const json = JSON.stringify(obj);
      if (json && json !== '{}') return json;
    } catch { /* circular reference or other — give up */ }
  }
  return null;
}

/**
 * Show an error toast notification to the user. (Fix 5.2 + 4.3)
 * Also logs to console for debugging.
 * Handles both standard JS Errors and structured AppError objects from
 * the Tauri backend IPC boundary.
 */
export function showErrorToast(message: string, error?: unknown): void {
  console.error(message, error);
  const detail = extractErrorMessage(error);
  const userMessage = detail ? `${message}: ${detail}` : message;
  useToastStore.getState().addToast(userMessage, 'error');
}
