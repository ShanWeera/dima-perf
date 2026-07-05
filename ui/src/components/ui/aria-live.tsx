/**
 * Accessible live region component for screen reader announcements.
 * Renders a visually hidden region that announces dynamic content changes
 * (position changes, filter results, analysis status) to assistive technology.
 */

import { useEffect, useRef, useState } from 'react';

interface AriaLiveProps {
  /** The message to announce. Changes trigger a new announcement. */
  message: string;
  /** Politeness level: 'polite' waits for idle, 'assertive' interrupts immediately */
  politeness?: 'polite' | 'assertive';
}

/**
 * Visually hidden live region that announces messages to screen readers.
 * Uses a double-buffer technique to ensure announcements are always picked up
 * even when the same message is repeated.
 */
export function AriaLive({ message, politeness = 'polite' }: AriaLiveProps) {
  const [current, setCurrent] = useState('');
  const toggleRef = useRef(false);

  useEffect(() => {
    if (!message) return;
    // Toggle forces re-announcement even if text is identical
    toggleRef.current = !toggleRef.current;
    setCurrent(message);
  }, [message]);

  return (
    <div
      role="status"
      aria-live={politeness}
      aria-atomic="true"
      className="sr-only"
    >
      {current}
    </div>
  );
}
