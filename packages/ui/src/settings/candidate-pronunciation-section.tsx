import { Row, Switch } from "../core/platform-controls";

export interface CandidatePronunciationSectionProps {
  value?: boolean;
  disabled?: boolean;
  /** 日文释义有没有罗马音：只有 macOS 和 Windows 宿主生成它，其他宿主只标英文音标，标题和说明也只讲英文音标。 */
  japaneseRomaji?: boolean;
  /** 音标表不随安装包、要在设置里下载一次（Linux）；开关下面另有下载行。 */
  downloadable?: boolean;
  onChange: (value: boolean) => void;
}

/** Reading switch for hosts that draw one after each gloss line: one row of the 多语言候选 group on the 表达 page. It is disabled while every gloss source is off, since there is then nothing to read, but keeps its value. */
export function CandidatePronunciationSection({
  value,
  disabled,
  japaneseRomaji = true,
  downloadable = false,
  onChange,
}: CandidatePronunciationSectionProps) {
  const description = japaneseRomaji
    ? "在释义后面标出怎么读：英文释义给音标，日文释义给罗马音。音标来自随输入法打包的离线词表，罗马音由系统生成，都不联网。需要先打开释义。"
    : downloadable
      ? "给英文释义标出音标。音标来自离线英文音标词典，下载一次后查词不联网。需要先打开释义。"
      : "给英文释义标出音标。音标来自随输入法打包的离线词表，不联网。需要先打开释义。";
  return (
    <Row title={japaneseRomaji ? "显示读音" : "显示英文音标"} description={description}>
      <Switch checked={value ?? false} disabled={disabled} onChange={onChange} />
    </Row>
  );
}
