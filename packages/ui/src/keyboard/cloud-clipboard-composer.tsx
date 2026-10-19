import { CLOUD_CLIPBOARD_MAX_UTF16 } from "../settings/cloud-clipboard-send";
import * as cloud from "./cloud-panel-style";

export interface CloudClipboardComposerProps {
  draft: string;
  enabled: boolean;
  busy: boolean;
  onDraftChange: (draft: string) => void;
  onUpload: () => void;
}

/** 受控的云剪贴板上传表单，草稿修订与上传结果由调用方管理。 */
export function CloudClipboardComposer({
  draft,
  enabled,
  busy,
  onDraftChange,
  onUpload,
}: CloudClipboardComposerProps) {
  return (
    <div className={cloud.clipboardAddRow}>
      <textarea
        className={cloud.clipboardTextArea}
        aria-label="待上传文本"
        value={draft}
        onChange={(event) => {
          onDraftChange(event.target.value);
        }}
        placeholder="输入要上传的文本"
        rows={3}
      />
      <small className={cloud.clipboardCount(draft.length > CLOUD_CLIPBOARD_MAX_UTF16)}>
        {draft.length} / {CLOUD_CLIPBOARD_MAX_UTF16}
      </small>
      <button
        type="button"
        className={cloud.clipboardSubmit}
        onClick={() => void onUpload()}
        disabled={
          !enabled || draft.trim().length === 0 || draft.length > CLOUD_CLIPBOARD_MAX_UTF16 || busy
        }
      >
        上传明确选择的文本
      </button>
    </div>
  );
}
