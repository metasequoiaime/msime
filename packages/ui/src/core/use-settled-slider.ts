import { useEffect, useState } from "react";
import { useLatestRef } from "./use-latest-ref";
import { useDebouncedCallback } from "./use-debounced-callback";

/** 滑块需要静止多久才保存其值。 */
const SLIDER_SAVE_DELAY_MS = 300;

/** 拖动时保留本地预览，静止后保存最后一个值；保存回调应同步更新外部值。 */
export function useSettledSlider(value: number, save: (value: number) => void) {
  const [moving, setMoving] = useState<number | null>(null);
  const saveRef = useLatestRef(save);
  const { schedule: saveLater, cancel: cancelSave } = useDebouncedCallback(
    (next: number, current: number) => {
      if (next !== current) saveRef.current(next);
      setMoving(null);
    },
    SLIDER_SAVE_DELAY_MS,
  );
  useEffect(() => {
    if (moving === null) {
      cancelSave();
      return;
    }
    // 保存与清除预览在同一 tick 里发生，让同步更新的外部值直接接替，避免闪现旧值。
    saveLater(moving, value);
  }, [cancelSave, moving, saveLater, value]);
  return [moving ?? value, setMoving] as const;
}
