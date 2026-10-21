import { ToolbarButton, ToolbarState } from "./FloatingToolbarLayout";
import { CandidateCanvasResource } from "./candidate/CandidateCanvasPolicy";

export interface ToolbarImageLook {
  readonly widthDip?: number | null;
  readonly heightDip?: number | null;
  readonly iconSizeDip?: number | null;
  readonly background?: string | null;
}
export interface ToolbarImageButton extends ToolbarImageLook {
  readonly visible?: boolean | null;
  readonly xDip?: number | null;
  readonly yDip?: number | null;
  readonly image?: string | null;
  readonly images: Record<string, string>;
}
export interface ToolbarImageSpec {
  readonly image: string;
  readonly imageWidthDip: number;
  readonly imageHeightDip: number;
  readonly buttonDefaults: ToolbarImageLook;
  readonly buttonFlow?: ToolbarImageFlow | null;
  readonly buttons: Record<string, ToolbarImageButton>;
}
export interface ToolbarImageFlow {
  readonly xDip: number;
  readonly yDip: number;
  readonly direction: string;
  readonly gapDip: number;
}
export interface ToolbarImageSlot {
  readonly key: string;
  readonly xDip: number;
  readonly yDip: number;
  readonly widthDip: number;
  readonly heightDip: number;
  readonly iconSizeDip: number;
  readonly background?: string | null;
  readonly image?: string | null;
}
export interface ToolbarImageLayout {
  readonly widthDip: number;
  readonly heightDip: number;
  readonly buttons: ToolbarImageSlot[];
}
export interface ToolbarImageResources {
  readonly key: string;
  readonly spec: ToolbarImageSpec;
  readonly resources: CandidateCanvasResource[];
}
export interface ToolbarImageScene extends ToolbarImageResources {
  readonly layout: ToolbarImageLayout;
}
export interface ToolbarImageLayoutReply {
  readonly ok: boolean;
  readonly value?: ToolbarImageLayout | null;
}

/** 宿主只适配按钮与当前状态，布局仍由共享 Rust 决定。 */
export class ToolbarImagePolicy {
  static key(button: ToolbarButton): string {
    switch (button) {
      case ToolbarButton.INPUT_MODE:
        return "status";
      case ToolbarButton.FULL_WIDTH:
        return "fullwidth";
      case ToolbarButton.PUNCTUATION:
        return "punctuation";
      case ToolbarButton.CHARACTER_SET:
        return "character_set";
      case ToolbarButton.EMOJI:
        return "emoji";
      case ToolbarButton.SCREEN_KEYBOARD:
        return "screen_keyboard";
      case ToolbarButton.SETTINGS:
        return "settings";
      default:
        return "";
    }
  }
  static button(key: string): ToolbarButton {
    switch (key) {
      case "status":
        return ToolbarButton.INPUT_MODE;
      case "fullwidth":
        return ToolbarButton.FULL_WIDTH;
      case "punctuation":
        return ToolbarButton.PUNCTUATION;
      case "character_set":
        return ToolbarButton.CHARACTER_SET;
      case "emoji":
        return ToolbarButton.EMOJI;
      case "screen_keyboard":
        return ToolbarButton.SCREEN_KEYBOARD;
      default:
        return ToolbarButton.SETTINGS;
    }
  }
  static shown(buttons: ToolbarButton[], state: ToolbarState): string[][] {
    const output: string[][] = [];
    for (const button of buttons) {
      const key: string = ToolbarImagePolicy.key(button);
      if (key.length === 0) continue;
      let value: string = "";
      if (key === "status")
        value = state.capsLock
          ? "caps"
          : state.english
            ? "en"
            : state.temporaryEnglish
              ? "en-candidate"
              : state.japanese
                ? "ja"
                : "cn";
      else if (key === "fullwidth") value = state.fullWidth ? "full" : "half";
      else if (key === "punctuation") value = state.chinesePunctuation ? "cn" : "en";
      else if (key === "character_set") value = state.traditional ? "traditional" : "simplified";
      output.push([key, value]);
    }
    return output;
  }
  /** JS 二维数组按物理像素分配；过大时回退普通条，不能让皮肤耗尽窗口进程内存。 */
  static mask(
    scene: ToolbarImageScene,
    unit: number,
    alpha: Uint8Array | null,
    sourceWidth: number,
    sourceHeight: number,
  ): number[][] | null {
    const width: number = Math.max(1, Math.round(scene.layout.widthDip * unit));
    const height: number = Math.max(1, Math.round(scene.layout.heightDip * unit));
    if (!Number.isFinite(unit) || unit <= 0 || width * height > 1000000) return null;
    const rows: number[][] = [];
    for (let y: number = 0; y < height; y++) rows.push(new Array<number>(width).fill(0));
    const fill = (left: number, top: number, right: number, bottom: number): void => {
      const first: number = Math.max(0, Math.floor(left));
      const last: number = Math.min(width, Math.ceil(right));
      for (
        let y: number = Math.max(0, Math.floor(top));
        y < Math.min(height, Math.ceil(bottom));
        y++
      ) {
        rows[y].fill(1, first, last);
      }
    };
    const backgroundWidth: number = scene.spec.imageWidthDip * unit;
    const backgroundHeight: number = scene.spec.imageHeightDip * unit;
    if (alpha === null || sourceWidth <= 0 || sourceHeight <= 0) {
      fill(0, 0, backgroundWidth, backgroundHeight);
    } else {
      for (let y: number = 0; y < sourceHeight; y++) {
        let x: number = 0;
        while (x < sourceWidth) {
          while (x < sourceWidth && alpha[y * sourceWidth + x] === 0) x++;
          const start: number = x;
          while (x < sourceWidth && alpha[y * sourceWidth + x] > 0) x++;
          if (start < x)
            fill(
              (start / sourceWidth) * backgroundWidth,
              (y / sourceHeight) * backgroundHeight,
              (x / sourceWidth) * backgroundWidth,
              ((y + 1) / sourceHeight) * backgroundHeight,
            );
        }
      }
    }
    for (const slot of scene.layout.buttons)
      fill(
        slot.xDip * unit,
        slot.yDip * unit,
        (slot.xDip + slot.widthDip) * unit,
        (slot.yDip + slot.heightDip) * unit,
      );
    return rows;
  }

  static paths(spec: ToolbarImageSpec): string[] {
    const paths: string[] = [spec.image];
    const add = (value: string | null | undefined): void => {
      if (typeof value === "string" && !paths.includes(value)) paths.push(value);
    };
    add(spec.buttonDefaults.background);
    for (const key of Object.keys(spec.buttons)) {
      const button: ToolbarImageButton = spec.buttons[key];
      add(button.background);
      add(button.image);
      for (const state of Object.keys(button.images)) add(button.images[state]);
    }
    return paths;
  }
}
