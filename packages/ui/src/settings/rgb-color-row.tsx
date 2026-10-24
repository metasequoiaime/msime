import { Row } from "../core/platform-controls";
import { skinColor, skinColorNumber } from "../keyboard/touch-keyboard-skin-design";
import * as skin from "../keyboard/touch-skin-style";

export interface RgbColorRowProps {
  title: string;
  value: number;
  hidden?: boolean;
  onChange: (value: number) => void;
}

/** 整数 RGB 颜色设置行：色块使用原生取色器，调用方持有数值颜色。 */
export function RgbColorRow({ title, value, hidden, onChange }: RgbColorRowProps) {
  const hex = skinColor(value).toUpperCase();
  return (
    <Row title={title} hidden={hidden}>
      <span className={skin.colorControl}>
        <span className={skin.colorValue} aria-hidden="true">
          {hex}
        </span>
        <input
          className={skin.colorSwatch}
          aria-label={title}
          type="color"
          value={hex}
          onChange={(event) => onChange(skinColorNumber(event.target.value))}
        />
      </span>
    </Row>
  );
}
