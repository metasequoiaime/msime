import { Row, Switch } from "../core/platform-controls";

export interface CandidatePronunciationSectionProps {
  value?: boolean;
  disabled?: boolean;
  /** 日文罗马音由系统生成（macOS 和 Windows）；其他宿主的罗马音和别的读音一样来自离线词表。 */
  japaneseRomaji?: boolean;
  /** 读音表不随安装包、要在设置里下载一次（Linux）；开关下面另有下载行。 */
  downloadable?: boolean;
  /** 键盘给英文候选本身标音标、不看释义开关（鸿蒙）；说明不写「需要先打开释义」。 */
  englishCandidates?: boolean;
  onChange: (value: boolean) => void;
}

/** Reading switch for hosts that draw one after each gloss line: one row of the 多语言候选 group on the 表达 page. It is disabled while every gloss source is off, since there is then nothing to read, but keeps its value. */
export function CandidatePronunciationSection({
  value,
  disabled,
  japaneseRomaji = true,
  downloadable = false,
  englishCandidates = false,
  onChange,
}: CandidatePronunciationSectionProps) {
  // 法、西、俄、德、韩语的读音是维基词典的国际音标，和离线释义出自同一份词典。
  const description = japaneseRomaji
    ? "在释义后面标出怎么读：英文释义给音标，日文释义给罗马音，法、西、俄、德、韩语释义给国际音标。音标来自随输入法打包的离线词表，罗马音由系统生成，都不联网。需要先打开释义。"
    : englishCandidates
      ? "在英文候选和释义下面标出读音：英文给音标，日文给罗马音，法、西、俄、德、韩语给国际音标。读音来自随输入法打包的离线词表，不联网。英文候选不用打开释义也会标出，中文候选要打开释义才有。"
      : downloadable
        ? "给释义标出读音：英文给音标，日文给罗马音，法、西、俄、德、韩语给国际音标。读音来自离线读音词典，下载一次后查词不联网。需要先打开释义。"
        : "给释义标出读音：英文给音标，日文给罗马音，法、西、俄、德、韩语给国际音标。读音来自随输入法打包的离线词表，不联网。需要先打开释义。";
  return (
    <Row title="显示读音" description={description}>
      <Switch checked={value ?? false} disabled={disabled} onChange={onChange} />
    </Row>
  );
}
