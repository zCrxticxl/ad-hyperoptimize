import { useEffect, useRef } from "react";

/**
 * Deep-link highlight (shared): when a page is opened with a target feature id
 * and its data is ready, scroll to + flash the matching `[data-focus-id]` row.
 * `ready` must flip to true once the page's data has loaded so the element
 * exists before we scroll to it.
 */
export function useFeatureFocus(focusId: string | undefined, ready: boolean) {
  const done = useRef(false);
  useEffect(() => {
    if (!focusId || done.current || !ready) return;
    done.current = true;
    requestAnimationFrame(() => {
      const el = document.querySelector(`[data-focus-id="${focusId}"]`);
      el?.scrollIntoView({ behavior: "smooth", block: "center" });
      el?.classList.add("feature-focus");
      setTimeout(() => el?.classList.remove("feature-focus"), 2200);
    });
  }, [focusId, ready]);
}
