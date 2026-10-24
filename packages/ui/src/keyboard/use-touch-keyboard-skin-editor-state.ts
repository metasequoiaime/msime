import { useState } from "react";
import {
  defaultTouchKeyboardSkinDesign,
  normalizeTouchKeyboardSkinDesign,
  readableSkinText,
  skinContrast,
  type TouchKeyboardSkinDesign,
} from "./touch-keyboard-skin-design";

export interface UseTouchKeyboardSkinEditorStateOptions {
  /** 当前设计由调用方持有；收到变更后以新值重新渲染。 */
  design: TouchKeyboardSkinDesign;
  onChange: (design: TouchKeyboardSkinDesign) => void;
}

/** 受控皮肤设计的编辑与最近 30 步历史，独立于编辑器布局和皮肤库。 */
export function useTouchKeyboardSkinEditorState({
  design,
  onChange,
}: UseTouchKeyboardSkinEditorStateOptions) {
  const [undo, setUndo] = useState<TouchKeyboardSkinDesign[]>([]);
  const [redo, setRedo] = useState<TouchKeyboardSkinDesign[]>([]);
  const apply = (next: TouchKeyboardSkinDesign, record = true) => {
    const normalized = normalizeTouchKeyboardSkinDesign(next);
    if (JSON.stringify(normalized) === JSON.stringify(design)) return;
    if (record) {
      setUndo((items) => [...items, design].slice(-30));
      setRedo([]);
    }
    onChange(normalized);
  };
  const patch = (value: Partial<TouchKeyboardSkinDesign>) => apply({ ...design, ...value });
  const stepBack = () => {
    const next = undo.at(-1);
    if (!next) return;
    setUndo((items) => items.slice(0, -1));
    setRedo((items) => [...items, design].slice(-30));
    apply(next, false);
  };
  const stepForward = () => {
    const next = redo.at(-1);
    if (!next) return;
    setRedo((items) => items.slice(0, -1));
    setUndo((items) => [...items, design].slice(-30));
    apply(next, false);
  };
  const optimizeContrast = () => {
    const black = Math.min(
      skinContrast(0, design.background),
      skinContrast(0, design.keyBackground),
    );
    const white = Math.min(
      skinContrast(0xffffff, design.background),
      skinContrast(0xffffff, design.keyBackground),
    );
    patch({
      keyForeground: readableSkinText(design.keyBackground),
      accent: black >= white ? 0 : 0xffffff,
    });
  };
  return {
    canUndo: undo.length > 0,
    canRedo: redo.length > 0,
    apply: (next: TouchKeyboardSkinDesign) => apply(next),
    patch,
    undo: stepBack,
    redo: stepForward,
    reset: () => apply(defaultTouchKeyboardSkinDesign),
    optimizeContrast,
  };
}

export type TouchKeyboardSkinEditorState = ReturnType<typeof useTouchKeyboardSkinEditorState>;
