import type { PointerEvent, PointerEventHandler } from "react";
import { keyboardKeyLabel, type KeyboardKey, type Modifier } from "./keyboard-input";
import type { TouchSkinKeyMaterial } from "./touch-keyboard-skin-design";
import * as surface from "./panel-surface-style";

export interface KeyboardKeyButtonProps {
  keyToRender: KeyboardKey;
  modifiers: ReadonlySet<Modifier>;
  material: TouchSkinKeyMaterial;
  flexGrow: number;
  disabled: boolean;
  onPointerDown: (event: PointerEvent<HTMLButtonElement>, key: KeyboardKey) => void;
  onPointerUp: PointerEventHandler<HTMLButtonElement>;
  onPointerCancel: PointerEventHandler<HTMLButtonElement>;
  onPointerLeave: PointerEventHandler<HTMLButtonElement>;
  onActivate: (key: KeyboardKey, displayed: string) => void;
}

/** 屏幕键盘的受控按键，保留键面、指针目标和点击来源约定。 */
export function KeyboardKeyButton({
  keyToRender,
  modifiers,
  material,
  flexGrow,
  disabled,
  onPointerDown,
  onPointerUp,
  onPointerCancel,
  onPointerLeave,
  onActivate,
}: KeyboardKeyButtonProps) {
  const shifted = modifiers.has("Shift") && keyToRender.label.length === 1;
  const label = keyboardKeyLabel(keyToRender, shifted);
  return (
    <button
      type="button"
      disabled={disabled}
      style={{ flexGrow }}
      aria-pressed={keyToRender.modifier ? modifiers.has(keyToRender.modifier) : undefined}
      data-key-material={material}
      className={`keyboard-key ${surface.keyboardKey({
        material,
        active: Boolean(keyToRender.modifier && modifiers.has(keyToRender.modifier)),
        wide: keyToRender.label.length > 1,
      })}`}
      onPointerDown={(event) => onPointerDown(event, keyToRender)}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerCancel}
      onPointerLeave={onPointerLeave}
      onClick={(event) => {
        // 普通指针由按下或 Windows 同键松开时发送；修饰键、Num Lock 与辅助激活的零 detail 点击在这里发送一次。
        if (keyToRender.modifier || keyToRender.virtualKey === 0x90 || event.detail === 0)
          onActivate(keyToRender, event.currentTarget.textContent ?? "");
      }}
    >
      <span data-key-label="">{label}</span>
      {keyToRender.hint && <span className={surface.keyboardKeyHint}>{keyToRender.hint}</span>}
    </button>
  );
}
