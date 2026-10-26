import { capabilityLabels } from "../settings/plugin-script-model";
import type { ScriptCapability } from "../settings/plugin-types";
import * as style from "./community-style";

/** 审核和安装前要特别提醒的能力：能改掉你输入到应用里的文字，或能把读到的文字长期留在这台电脑上。服务端审核列表标红的也是这两项。 */
export const flaggedScriptCapabilities: readonly string[] = ["rewrite_commit", "persist_data"];

/** 能力名在授权页上的白话标题；本客户端还不认识的能力没有标题。 */
function capabilityTitle(name: string): string | null {
  return Object.prototype.hasOwnProperty.call(capabilityLabels, name)
    ? capabilityLabels[name as ScriptCapability].title
    : null;
}

/** 卡片上的一行摘要：申请的能力标题，用顿号隔开。 */
export function scriptCapabilitySummary(permissions: readonly string[] | undefined): string {
  if (!permissions || permissions.length === 0) return "不申请任何能力";
  return permissions.map((name) => capabilityTitle(name) ?? name).join("、");
}

/** 社区脚本插件详情里的能力说明：逐项用授权页的白话列出插件申请的能力，标出要特别留意的两项，并说明安装后不会运行。 */
export function CommunityPluginCapabilities({ permissions }: { permissions: readonly string[] }) {
  const unknown = permissions.filter((name) => capabilityTitle(name) === null);
  return (
    <section className={style.confirmation} aria-label="脚本插件申请的能力">
      <p>安装后不会运行，要在脚本插件页授权。授权时你可以只勾选其中一部分能力，也可以随时撤销。</p>
      {permissions.length === 0 ? (
        <p>这个插件不申请任何能力：它只能知道发生了什么事件，看不到也改不了你打的字。</p>
      ) : (
        <ul className="m-0 flex list-disc flex-col gap-1.5 pl-5 text-secondary">
          {permissions.map((name) => {
            const label = Object.prototype.hasOwnProperty.call(capabilityLabels, name)
              ? capabilityLabels[name as ScriptCapability]
              : null;
            const flagged = flaggedScriptCapabilities.includes(name);
            return (
              <li key={name}>
                <strong className={flagged ? "text-danger" : undefined}>
                  {flagged && "注意："}
                  {label?.title ?? `未知能力 ${name}`}
                </strong>
                {label && ` —— ${label.description}`}
              </li>
            );
          })}
        </ul>
      )}
      {unknown.length > 0 && (
        <p>这个插件申请了这个版本的输入法还不认识的能力，安装会失败，请先更新输入法。</p>
      )}
    </section>
  );
}
