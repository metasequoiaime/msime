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
import ollama from "../assets/providers/ollama.svg";
import { FluentIcon } from "../core/fluent-icons";

const icons: Record<string, string> = {
  everyapi,
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
  ollama,
};

/** 保留品牌配色；需要底色的标志使用固定底色，深浅主题和选中行都不改写图标颜色。 */
export function AiProviderIcon({ provider }: { provider: string }) {
  const icon = icons[provider];
  return (
    <span
      className="ai-provider-icon"
      aria-hidden="true"
      data-provider-icon={provider}
      data-brand-tile={
        icon ? (["kimi", "openrouter"].includes(provider) ? "dark" : "light") : undefined
      }
    >
      {icon ? (
        <img src={icon} alt="" width="20" height="20" />
      ) : (
        <FluentIcon name="link" size={20} />
      )}
    </span>
  );
}
