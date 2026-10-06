import { useEffect, useRef, type HTMLAttributes } from "react";
import { useMountedRef } from "../settings/use-mounted-ref";

export function usePanelDrag(
  client: { beginWindowDrag?(): Promise<void> },
  onFailure: () => void,
): HTMLAttributes<HTMLElement> {
  const pendingDrag = useRef<{ id: number; x: number; y: number } | null>(null);
  const mounted = useMountedRef();
  const reset = () => {
    pendingDrag.current = null;
  };
  useEffect(() => {
    window.addEventListener("blur", reset);
    return () => {
      reset();
      window.removeEventListener("blur", reset);
    };
  }, [client]);
  return {
    onPointerDown(event) {
      reset();
      if (
        !client.beginWindowDrag ||
        event.button !== 0 ||
        (event.target as Element).closest("button")
      )
        return;
      pendingDrag.current = { id: event.pointerId, x: event.clientX, y: event.clientY };
    },
    onPointerMove(event) {
      const pending = pendingDrag.current;
      if (!pending || pending.id !== event.pointerId) return;
      if (event.buttons !== 1) {
        reset();
        return;
      }
      if (Math.abs(event.clientX - pending.x) + Math.abs(event.clientY - pending.y) < 2) return;
      reset();
      void (async () => {
        try {
          await client.beginWindowDrag?.();
        } catch {
          if (mounted.current) onFailure();
        }
      })();
    },
    onPointerUp: reset,
    onPointerCancel: reset,
    onPointerLeave: reset,
  };
}
