import { useEffect, useRef } from "react";

/** Returns a ref that tracks whether the owning component is still mounted. */
export function useMountedRef() {
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  return mounted;
}
