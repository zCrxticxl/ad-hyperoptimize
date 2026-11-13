import { useEffect } from "react";

/** Scroll to the requested feature once its row is available. */
export function useFeatureFocus(focusId: string | undefined, ready: boolean) {
  useEffect(() => {
    if (!focusId || !ready) return;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let el: Element | null = null;
    const frame = requestAnimationFrame(() => {
      el = document.querySelector(`[data-focus-id="${CSS.escape(focusId)}"]`);
      el?.scrollIntoView({ behavior: "smooth", block: "center" });
      el?.classList.add("feature-focus");
      timer = setTimeout(() => el?.classList.remove("feature-focus"), 2200);
    });
    return () => {
      cancelAnimationFrame(frame);
      clearTimeout(timer);
      el?.classList.remove("feature-focus");
    };
  }, [focusId, ready]);
}
