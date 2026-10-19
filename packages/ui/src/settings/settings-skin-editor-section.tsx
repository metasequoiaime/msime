import { CandidateSkinEditor } from "../skin/candidate-skin-editor";
import { useSkinCatalog } from "../skin/external-skins";
import { applyCandidateSkin, skinSlot } from "../theme/global-theme";
import { createSettingsDraftActions } from "./settings-draft-actions";
import type { SettingsPageModel } from "./use-settings-page-model";
import type {
  useSettingsCommunityViews,
  UseSettingsCommunityViewsOptions,
} from "./use-settings-community-views";

export interface SettingsSkinEditorSectionProps {
  model: Pick<
    SettingsPageModel,
    "client" | "page" | "draft" | "setDraft" | "host" | "mobilePlatform"
  >;
  views: Pick<ReturnType<typeof useSettingsCommunityViews>, "desktopTabs" | "skinEditorShown">;
  onSkinViewChange: UseSettingsCommunityViewsOptions["onSkinViewChange"];
}

/** 桌面主题页的「制作皮肤」标签。它和社区图库一样画在偏好表单之外：编辑器有自己的保存，不随偏好自动保存；只有「立即使用」改偏好草稿。离开这一页或这个标签时保持挂载，正在编辑的草稿不丢。 */
export function SettingsSkinEditorSection({
  model,
  views,
  onSkinViewChange,
}: SettingsSkinEditorSectionProps) {
  const { client, page, draft, setDraft, host, mobilePlatform } = model;
  const editor =
    host?.candidate_skin_editor === true && !mobilePlatform
      ? client.candidateSkinEditor
      : undefined;
  const skins = useSkinCatalog(
    editor ? client.scanSkinCatalog : undefined,
    undefined,
    false,
    views.skinEditorShown,
  );
  if (!editor) return null;
  const packages = skins.catalog?.packages ?? [];
  const { onPreferencesChange } = createSettingsDraftActions({ setDraft });
  const customTheme = draft?.custom_theme;
  return (
    <div hidden={page !== "skin"}>
      {views.desktopTabs.panel(
        "create",
        <div className="mt-4">
          <CandidateSkinEditor
            client={editor}
            catalog={skins.catalog}
            readImage={client.readSkinImage}
            inUse={(id) =>
              draft?.global_theme === "custom" &&
              (customTheme?.candidate_skin === id || customTheme?.candidate_skin_dark === id)
            }
            // 与「本地皮肤」卡片的开关相同：皮肤放进它 base 所属的槽位，另一个槽位不动。
            onUse={(id, base) =>
              onPreferencesChange({
                global_theme: "custom",
                custom_theme: applyCandidateSkin(customTheme, id, base, (other) => {
                  const skin = packages.find((entry) => entry.id === other);
                  return skin ? skinSlot(skin.base) : undefined;
                }),
              })
            }
            onShowLocal={() => onSkinViewChange("mine")}
          />
        </div>,
      )}
    </div>
  );
}
