import everyapi from "../assets/providers/everyapi.svg";
import openai from "../assets/providers/openai.svg";
import claude from "../assets/providers/claude.svg";
import gemini from "../assets/providers/gemini.svg";
import deepseek from "../assets/providers/deepseek.svg";
import qwen from "../assets/providers/qwen.svg";
import kimi from "../assets/providers/kimi.svg";
import zhipu from "../assets/providers/zhipu.svg";
import siliconcloud from "../assets/providers/siliconcloud.svg";
import groq from "../assets/providers/groq.svg";
import openrouter from "../assets/providers/openrouter.svg";
import { FluentIcon } from "../core/fluent-icons";

const icons: Record<string, string> = {
  openai,
  anthropic: claude,
  gemini,
  deepseek,
  qwen,
  kimi,
  zhipu,
  siliconflow: siliconcloud,
  groq,
  openrouter,
};

/** 品牌图形随应用打包；单色标志继承文字颜色，深浅主题下都保持可见。 */
export function AiProviderIcon({ provider }: { provider: string }) {
  const icon = icons[provider];
  return (
    <span className="ai-provider-icon" aria-hidden="true" data-provider-icon={provider}>
      {provider === "everyapi" ? (
        <img src={everyapi} alt="" width="20" height="20" />
      ) : icon ? (
        <span style={{ maskImage: `url("${icon}")`, WebkitMaskImage: `url("${icon}")` }} />
      ) : (
        <FluentIcon name="link" size={20} />
      )}
    </span>
  );
}
