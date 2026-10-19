import * as surface from "./panel-surface-style";

export interface VoiceResultsProps {
  text: string;
  exceedsSubmitLimit: boolean;
  inputAvailable: boolean;
  copyAvailable: boolean;
  busy: boolean;
  submitting: boolean;
  copying: boolean;
  onTextChange: (text: string) => void;
  onSubmit: (copyOnly?: boolean) => void;
  onClear: () => void;
}

/** 受控的语音结果编辑与操作区，保留提交、复制和清空各自的许可条件。 */
export function VoiceResults({
  text,
  exceedsSubmitLimit,
  inputAvailable,
  copyAvailable,
  busy,
  submitting,
  copying,
  onTextChange,
  onSubmit,
  onClear,
}: VoiceResultsProps) {
  return (
    <>
      <textarea
        className={surface.voiceTextArea}
        aria-label="识别结果"
        aria-describedby={exceedsSubmitLimit ? "voice-result-limit" : undefined}
        aria-invalid={exceedsSubmitLimit || undefined}
        value={text}
        maxLength={4096}
        onChange={(event) => onTextChange(event.target.value)}
        placeholder="识别结果会显示在这里"
        rows={4}
      />
      {exceedsSubmitLimit && (
        <p id="voice-result-limit" className={surface.voiceNote} role="status">
          内容超过单次提交长度，请精简或复制结果后手动粘贴。
        </p>
      )}
      <button
        type="button"
        className={surface.voiceSubmit}
        onClick={() => void onSubmit()}
        disabled={!text || exceedsSubmitLimit || !inputAvailable || busy || submitting}
      >
        {submitting && !copying ? "正在提交…" : "提交到当前窗口"}
      </button>
      {copyAvailable && (
        <button
          type="button"
          onClick={() => void onSubmit(true)}
          disabled={!text || busy || submitting}
        >
          {copying ? "正在复制…" : "复制结果"}
        </button>
      )}
      <button type="button" onClick={onClear} disabled={!text || busy}>
        清空结果
      </button>
    </>
  );
}
