import * as doc from "./document-style";

export interface HelpFeedbackSectionProps {
  visible: boolean;
  onHelp: () => void;
  onFeedback: () => void;
}

/** Mobile navigation links for help and feedback pages. */
export function HelpFeedbackSection({ visible, onHelp, onFeedback }: HelpFeedbackSectionProps) {
  if (!visible) return null;

  return (
    <div className={`section ${doc.linkList}`} aria-label="帮助与反馈">
      <button type="button" className={doc.linkRow} onClick={onHelp}>
        <span className={doc.linkTitle}>使用帮助</span>
        <span aria-hidden="true">›</span>
      </button>
      <button type="button" className={doc.linkRow} onClick={onFeedback}>
        <span className={doc.linkTitle}>反馈问题与建议</span>
        <span aria-hidden="true">›</span>
      </button>
    </div>
  );
}
