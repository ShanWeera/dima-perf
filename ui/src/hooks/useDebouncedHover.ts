/**
 * useDebouncedHover - Provides rAF-debounced hover callbacks.
 *
 * Mouse movement over dense SVG elements fires hundreds of events/sec.
 * This hook batches updates to at most once per animation frame (~60Hz),
 * preventing cascading re-renders in the 3D viewer and other subscribers.
 */

import { useCallback, useEffect, useRef } from 'react';

export function useDebouncedHover<T>(
  onHover: (item: T | null) => void
): {
  handleMouseEnter: (item: T) => void;
  handleMouseLeave: () => void;
} {
  const rafRef = useRef<number | null>(null);

  // On unmount: cancel pending rAF and clear any lingering hover state (Fix 5.79).
  // Without the null call, `hoveredFeature` persists in the Zustand store and
  // the 3D viewer continues highlighting a feature from the now-destroyed panel.
  useEffect(() => {
    return () => {
      if (rafRef.current !== null) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = null;
      }
      onHover(null);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- cleanup-only, onHover is stable
  }, []);

  const handleMouseEnter = useCallback(
    (item: T) => {
      if (rafRef.current !== null) {
        cancelAnimationFrame(rafRef.current);
      }
      rafRef.current = requestAnimationFrame(() => {
        onHover(item);
        rafRef.current = null;
      });
    },
    [onHover]
  );

  const handleMouseLeave = useCallback(() => {
    if (rafRef.current !== null) {
      cancelAnimationFrame(rafRef.current);
    }
    rafRef.current = requestAnimationFrame(() => {
      onHover(null);
      rafRef.current = null;
    });
  }, [onHover]);

  return { handleMouseEnter, handleMouseLeave };
}
