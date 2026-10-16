import { useEffect, useMemo, useState } from "react";
import { boundedGraphemes } from "../core/text";
import { useAsyncActionRunner } from "../core/use-async-action";
import { useAsyncGeneration } from "../settings/use-async-generation";
import type { PluginCatalogResult } from "../settings/plugin-types";
import { packKindLabel } from "../settings/plugin-catalog-helpers";
import { communityNeedsSignIn, communityPluginMessage } from "./community-helpers";
import { communityPublishFields } from "./community-publish-validation";
import {
  communityPluginKinds,
  type CommunityPluginKind,
  type CommunityPlugin,
  type CommunityPluginPackPreview,
  type CommunityPluginClient,
} from "./community-plugin-model";
import { useCommunityPublicationDraft } from "./use-community-publication-draft";

export interface CommunityPluginPublicationOptions {
  client: Pick<CommunityPluginClient, "packPreview" | "publish">;
  localPlugins: () => Promise<PluginCatalogResult>;
  onPublished: (plugin: CommunityPlugin) => void | Promise<void>;
}

/** `label` 是类型显示名：本机的包知道自己是不是旋律音效包，所以用 `packKindLabel`；社区列表的条目不带 `mode`，只能用 `kindLabels`。 */
type LocalPluginOption = {
  key: string;
  kind: CommunityPluginKind;
  label: string;
  id: string;
  name: string;
};

function optionKey(kind: CommunityPluginKind, id: string): string {
  return `${kind}/${id}`;
}

/** 插件发布的目录、预检与草稿状态；成功后的关闭和图库刷新由调用方处理。 */
export function useCommunityPluginPublication({
  client,
  localPlugins,
  onPublished,
}: CommunityPluginPublicationOptions) {
  const [options, setOptions] = useState<LocalPluginOption[]>([]);
  const [optionsLoading, setOptionsLoading] = useState(true);
  const [selection, setSelection] = useState("");
  const [pack, setPack] = useState<CommunityPluginPackPreview | null>(null);
  const [packError, setPackError] = useState("");
  const [packLoading, setPackLoading] = useState(false);
  const {
    name,
    description,
    agreed,
    publicationId,
    setName,
    setDescription,
    setAgreed,
    onNameChange,
    onDescriptionChange,
    onAgreedChange,
    resetPublication,
  } = useCommunityPublicationDraft();
  const [error, setError] = useState("");
  const [signInRequired, setSignInRequired] = useState(false);
  const {
    busy,
    generation: clientGeneration,
    running: actionRunning,
    run: runAsyncAction,
  } = useAsyncActionRunner(setError, undefined, client, localPlugins);

  useEffect(() => {
    const generation = clientGeneration.current;
    setOptionsLoading(true);
    void localPlugins()
      .then((catalog) => {
        if (generation !== clientGeneration.current) return;
        const loaded: LocalPluginOption[] = [];
        for (const item of catalog.packages) {
          if (item.builtin) continue;
          // 桌面的目录里有脚本插件，它们不能发布到社区（`PUBLISHABLE_KINDS` 里没有 script）。
          if (!communityPluginKinds.includes(item.kind)) continue;
          loaded.push({
            key: optionKey(item.kind, item.id),
            kind: item.kind,
            label: packKindLabel(item),
            id: item.id,
            name: item.name,
          });
        }
        setOptions(loaded);
        setSelection((current) =>
          current && loaded.some((item) => item.key === current) ? current : (loaded[0]?.key ?? ""),
        );
      })
      .catch(() => {
        if (generation === clientGeneration.current) setError("读取本地插件失败，请重试。");
      })
      .finally(() => {
        if (generation === clientGeneration.current) setOptionsLoading(false);
      });
  }, [client, localPlugins, clientGeneration]);

  const chosen = useMemo(
    () => options.find((item) => item.key === selection) ?? null,
    [options, selection],
  );
  const packGeneration = useAsyncGeneration(client, chosen);

  useEffect(() => {
    const generation = packGeneration.current;
    setPack(null);
    setPackError("");
    setAgreed(false);
    resetPublication();
    if (!chosen) {
      setPackLoading(false);
      return;
    }
    setPackLoading(true);
    void client
      .packPreview(chosen.kind, chosen.id)
      .then((value) => {
        if (generation !== packGeneration.current) return;
        setPack(value);
        setName(boundedGraphemes(value.suggestedName, 32));
        setDescription(value.suggestedDescription);
      })
      .catch((packFailure) => {
        if (generation === packGeneration.current)
          setPackError(communityPluginMessage(packFailure));
      })
      .finally(() => {
        if (generation === packGeneration.current) setPackLoading(false);
      });
  }, [client, chosen, packGeneration]);

  const { normalizedName, normalizedDescription, nameValid, descriptionValid } =
    communityPublishFields(name, description);
  const ready = Boolean(pack) && !packLoading && nameValid && descriptionValid && agreed;

  const submit = async () => {
    if (busy || actionRunning.current || !ready || !chosen) return;
    setSignInRequired(false);
    await runAsyncAction(
      async (isCurrent) => {
        const published = await client.publish(
          chosen.kind,
          chosen.id,
          publicationId,
          normalizedName,
          normalizedDescription,
        );
        if (!isCurrent()) return;
        await onPublished(published);
      },
      {
        formatError: (publishError) => communityPluginMessage(publishError, true),
        onError: (publishError) => setSignInRequired(communityNeedsSignIn(publishError)),
      },
    );
  };

  return {
    options,
    optionsLoading,
    selection,
    pack,
    packError,
    packLoading,
    name,
    description,
    agreed,
    busy,
    error,
    signInRequired,
    ready,
    selectPlugin: setSelection,
    onNameChange,
    onDescriptionChange,
    onAgreedChange,
    submit,
  };
}
