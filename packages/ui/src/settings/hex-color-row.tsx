import { useEffect, useState } from "react";
import { Row } from "../core/platform-controls";
import { colorSwatch } from "../keyboard/touch-skin-style";
import { normalizedColor } from "../theme/global-theme";
import * as settings from "./settings-style";

export interface HexColorRowProps {
  title: string;
  /** 调用方持有的规范化颜色：`#RRGGBB` 或 `#RRGGBBAA`。 */
  value: string;
  /** 仅通知有效的十六进制颜色，调用方以新值重新渲染。 */
  onChange: (value: string) => void;
}

/** 原生取色器选 RGB，文本框支持透明度；无效草稿失焦后恢复当前受控颜色。 */
export function HexColorRow({ title, value, onChange }: HexColorRowProps) {
  const [text, setText] = useState(value);
  useEffect(() => setText(value), [value]);
  const invalid = normalizedColor(text) === null || !text.trim().startsWith("#");
  const alpha = value.length === 9 ? value.slice(7) : "";
  return (
    <Row title={title}>
      <span className={settings.skinEditorColor}>
        <input
          type="color"
          className={colorSwatch}
          aria-label={`${title}取色器`}
          value={value.slice(0, 7).toLowerCase()}
          onChange={(event) => onChange(`${event.target.value.toUpperCase()}${alpha}`)}
        />
        <input
          type="text"
          className={settings.skinEditorHex}
          aria-label={`${title}色值`}
          aria-invalid={invalid}
          spellCheck={false}
          maxLength={9}
          value={text}
          onChange={(event) => {
            const next = event.target.value.trim();
            setText(next);
            const normalized = next.startsWith("#") ? normalizedColor(next) : null;
            if (normalized) onChange(normalized);
          }}
          onBlur={() => setText(value)}
        />
        {/* 原生取色器画不出透明度，半透明的颜色在色值后面写出不透明度。 */}
        <span className={settings.skinEditorAlpha} aria-hidden="true">
          {alpha ? `${Math.round((parseInt(alpha, 16) / 255) * 100)}%` : ""}
        </span>
      </span>
    </Row>
  );
}
