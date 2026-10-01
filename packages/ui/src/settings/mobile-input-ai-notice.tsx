import { GroupList, Row } from "../core/platform-controls";

export interface MobileInputAiNoticeProps {
  grouped?: boolean;
  onOpenAi: () => void;
}

/** Mobile keyboard AI explanation and shortcut into the AI settings page. */
export function MobileInputAiNotice({ grouped = false, onOpenAi }: MobileInputAiNoticeProps) {
  const description =
    "复制对方的话，切换到高情商回复键盘，点“粘贴”后选择回复风格。支持帮你回、帮润色和换一句，点选回复插入聊天输入框。";
  if (grouped) {
    return (
      <GroupList title="高情商回复">
        <Row title="高情商回复" description={description}>
          <button type="button" className="secondary" onClick={onOpenAi}>
            配置键盘 AI
          </button>
        </Row>
      </GroupList>
    );
  }
  return (
    <div className="section input-ai-info">
      <div className="section-title">高情商回复</div>
      <p>{description}</p>
      <button type="button" className="secondary" onClick={onOpenAi}>
        配置键盘 AI
      </button>
    </div>
  );
}
