import type { PointerEventHandler } from "react";
import type { InkStroke } from "./panel-client";
import * as surface from "./panel-surface-style";

export interface HandwritingInkEditorProps {
  /** 用于显示的完整笔迹，包含当前尚未完成的笔画。 */
  strokes: readonly InkStroke[];
  drawing: boolean;
  disabled: boolean;
  recognizing: boolean;
  canUndo: boolean;
  canRedo: boolean;
  canClear: boolean;
  canRecognize: boolean;
  onStart: PointerEventHandler<SVGSVGElement>;
  onMove: PointerEventHandler<SVGSVGElement>;
  onEnd: PointerEventHandler<SVGSVGElement>;
  onCancel: PointerEventHandler<SVGSVGElement>;
  onUndo: () => void;
  onRedo: () => void;
  onClear: () => void;
  onRecognize?: () => void;
}

/** 手写笔迹编辑区；操作可用条件和笔画生命周期由调用方提供。 */
export function HandwritingInkEditor({
  strokes,
  drawing,
  disabled,
  recognizing,
  canUndo,
  canRedo,
  canClear,
  canRecognize,
  onStart,
  onMove,
  onEnd,
  onCancel,
  onUndo,
  onRedo,
  onClear,
  onRecognize,
}: HandwritingInkEditorProps) {
  return (
    <section className={surface.inkSection}>
      <svg
        className={surface.inkCanvas}
        data-drawing={drawing}
        tabIndex={0}
        viewBox="0 0 420 420"
        onPointerDown={onStart}
        onPointerMove={onMove}
        onPointerUp={onEnd}
        onPointerCancel={onCancel}
        onLostPointerCapture={onCancel}
        aria-label="手写画布"
        aria-disabled={disabled}
      >
        <g className={surface.inkGuides} aria-hidden="true">
          <path d="M210 20V400 M20 210H400 M20 20L400 400 M400 20L20 400" />
        </g>
        {strokes.map((stroke, index) =>
          stroke.points.length === 1 ? (
            <circle key={index} cx={stroke.points[0].x} cy={stroke.points[0].y} r={2} />
          ) : (
            <polyline key={index} points={stroke.points.map(({ x, y }) => `${x},${y}`).join(" ")} />
          ),
        )}
        {!strokes.length && (
          <text x="210" y="215" textAnchor="middle">
            请在这里书写
          </text>
        )}
      </svg>
      <div className={surface.inkActions}>
        <button
          type="button"
          className={surface.inkAction}
          onClick={onUndo}
          disabled={disabled || !canUndo}
          aria-keyshortcuts="Control+z Meta+z"
        >
          ↶ 撤销
        </button>
        <button
          type="button"
          className={surface.inkAction}
          onClick={onRedo}
          disabled={disabled || !canRedo}
          aria-keyshortcuts="Control+Shift+z Meta+Shift+z Control+y"
        >
          ↷ 重做
        </button>
        <button
          type="button"
          className={surface.inkAction}
          onClick={onClear}
          disabled={disabled || !canClear}
        >
          × 重写
        </button>
        {onRecognize && (
          <button
            type="button"
            className={surface.inkAction}
            onClick={onRecognize}
            disabled={disabled || !canRecognize}
          >
            {recognizing ? "识别中…" : "重新识别"}
          </button>
        )}
      </div>
    </section>
  );
}
