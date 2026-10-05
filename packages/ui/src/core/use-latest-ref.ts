import { useRef, type MutableRefObject } from "react";

/** Keeps a stable ref whose value follows the latest render value. */
export function useLatestRef<T>(value: T): MutableRefObject<T> {
  const latest = useRef(value);
  latest.current = value;
  return latest;
}
