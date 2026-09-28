import { utf8Length } from "./Utf8";
import { TextPolicy } from "./TextPolicy";

export interface ReplyStyle {
  readonly id: string;
  readonly title: string;
}
export interface ReplyRequest {
  readonly source: string;
  readonly style: string;
  readonly prompt: string;
}

export class ReplyKeyboardPolicy {
  static readonly MAX_SOURCE_UTF8: number = 10000;
  static readonly MAX_RESULT_UTF8: number = 4096;
  static readonly MAX_RESULTS: number = 3;
  static readonly STYLES: ReplyStyle[] = [
    { id: "专属回复", title: "😁 专属回复" },
    { id: "暖心关怀", title: "🥰 暖心关怀" },
    { id: "捧场王", title: "📣 捧场王" },
    { id: "恋人", title: "😍 恋人" },
    { id: "幽默风趣", title: "🌪 幽默风趣" },
    { id: "成熟稳重", title: "👔 成熟稳重" },
    { id: "土味情话", title: "💬 土味情话" },
    { id: "高情商", title: "🤩 高情商" },
    { id: "委婉拒绝", title: "🙌 委婉拒绝" },
  ];
  static source(value: string): string | null {
    const trimmed: string = value.trim();
    if (
      trimmed.length === 0 ||
      utf8Length(trimmed) > ReplyKeyboardPolicy.MAX_SOURCE_UTF8 ||
      TextPolicy.hasControl(trimmed)
    )
      return null;
    return trimmed;
  }
  static request(
    source: string,
    style: string,
    extraPrompt: string = "",
    polish: boolean = false,
  ): ReplyRequest | null {
    const normalized: string | null = ReplyKeyboardPolicy.source(source);
    const selected: string = style.trim();
    if (normalized === null || selected.length === 0 || TextPolicy.hasControl(selected))
      return null;
    const prompt: string = extraPrompt.trim();
    if (
      prompt.length > 0 &&
      (utf8Length(prompt) > 64 * 1024 || TextPolicy.hasControl(prompt))
    )
      return null;
    const instruction: string = polish
      ? `请以${selected}的语气润色用户文字，保持原意，不编造事实或承诺。只输出一条简短自然的成稿，不加标题、解释或引号。`
      : `对方发来以下内容，请拟写一条${selected}风格的高情商回复。尊重对方且有边界，不编造事实、关系或承诺。只输出一条简短自然、可以直接发送的回复，不加标题、解释或引号。`;
    return {
      source: normalized,
      style: selected,
      prompt: `${prompt.length > 0 ? prompt + "\n" : ""}${instruction}`,
    };
  }
  static results(values: string[] | null): string[] {
    if (values === null) return [];
    const result: string[] = [];
    for (const value of values) {
      if (
        typeof value !== "string" ||
        value.trim().length === 0 ||
        utf8Length(value) > ReplyKeyboardPolicy.MAX_RESULT_UTF8 ||
        TextPolicy.hasControl(value)
      )
        continue;
      if (!result.includes(value)) result.push(value);
      if (result.length === ReplyKeyboardPolicy.MAX_RESULTS) break;
    }
    return result;
  }
}
