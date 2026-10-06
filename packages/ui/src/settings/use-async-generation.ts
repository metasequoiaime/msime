import { useEffect, useRef, type MutableRefObject } from "react";

/** Tracks the current owner generation for asynchronous settings work. */
export function useAsyncGeneration(...owners: readonly unknown[]): MutableRefObject<number> {
  const generation = useRef(0);

  useEffect(() => {
    generation.current++;
    return () => {
      generation.current++;
    };
  }, owners);

  return generation;
}
