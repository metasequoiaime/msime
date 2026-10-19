import type { KeyboardEventHandler, ReactNode } from "react";
import { FluentIcon } from "../core/fluent-icons";
import { StatusMessage } from "../core/status-message";
import { HandwritingCandidateButton } from "./handwriting-candidate-button";
import { PanelActivationButtons, type PanelActivationMode } from "./panel-activation-buttons";
import * as surface from "./panel-surface-style";

export interface HandwritingResultsProps {
  candidates: readonly string[];
  mode: PanelActivationMode;
  copyAvailable: boolean;
  inputAvailable: boolean;
  disabled: boolean;
  notice: string;
  onModeChange: (mode: PanelActivationMode) => void;
  onNavigate: KeyboardEventHandler<HTMLDivElement>;
  onChoose: (candidate: string, copyOnly?: boolean) => void;
  /** 状态提示之前的模型下载提示等附加内容。 */
  children?: ReactNode;
}

/** 受控的手写识别结果区，复用模式按钮与候选按钮的公共契约。 */
export function HandwritingResults({
  candidates,
  mode,
  copyAvailable,
  inputAvailable,
  disabled,
  notice,
  onModeChange,
  onNavigate,
  onChoose,
  children,
}: HandwritingResultsProps) {
  return (
    <section className={surface.recognitionSection}>
      <h2>识别结果</h2>
      <div className={surface.handwritingActions} role="group" aria-label="点击手写候选的操作">
        <span>点击候选：</span>
        <PanelActivationButtons
          mode={mode}
          onModeChange={onModeChange}
          disabled={disabled}
          copyDisabled={!copyAvailable}
          inputAvailable={inputAvailable}
        />
      </div>
      <div
        className={surface.candidateGrid}
        // 为候选网格命名，便于辅助技术和键盘导航定位整个分组。
        role="group"
        aria-label="识别候选"
        onKeyDown={onNavigate}
      >
        {candidates.map((candidate) => (
          <div className={surface.candidate} key={candidate}>
            <HandwritingCandidateButton
              candidate={candidate}
              copy={mode === "copy"}
              disabled={disabled}
              onChoose={() => void onChoose(candidate, mode === "copy")}
              onCopy={copyAvailable ? () => void onChoose(candidate, true) : undefined}
            />
            {mode === "copy" && inputAvailable ? (
              <button
                type="button"
                className={surface.candidateCopy}
                aria-label={`输入候选 ${candidate}`}
                disabled={disabled}
                onClick={() => void onChoose(candidate)}
              >
                输入
              </button>
            ) : (
              mode === "input" &&
              copyAvailable && (
                <button
                  type="button"
                  className={surface.candidateCopy}
                  aria-label={`复制候选 ${candidate}`}
                  disabled={disabled}
                  onClick={() => void onChoose(candidate, true)}
                >
                  复制
                </button>
              )
            )}
          </div>
        ))}
      </div>
      {!candidates.length && (
        <div className={surface.handwritingEmpty} aria-hidden="true">
          <span>
            <FluentIcon name="pen" size={26} />
          </span>
          <p>写下一个字，候选会出现在这里</p>
          <small>每写完一笔，自动更新识别结果</small>
        </div>
      )}
      {candidates.length > 0 && <div className="flex-1" />}
      {children}
      <StatusMessage role="status">{notice}</StatusMessage>
    </section>
  );
}
