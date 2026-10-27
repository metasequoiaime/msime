import { useLayoutEffect, useRef, type MutableRefObject } from "react";

/** 在布局阶段切换异步归属，确保 DOM 可交互前旧请求已经失效。 */
export function useAsyncGeneration(...owners: readonly unknown[]): MutableRefObject<number> {
  const generation = useRef(0);

  useLayoutEffect(() => {
    generation.current++;
    return () => {
      generation.current++;
    };
  }, owners);

  return generation;
}
