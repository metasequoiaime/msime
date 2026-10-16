import { useEffect, useState } from "react";
import { useAsyncActionRunner } from "../core/use-async-action";
import type {
  CustomSkinLibraryClient,
  SavedTouchKeyboardSkin,
} from "../keyboard/touch-keyboard-skin-design";
import { communityNeedsSignIn, communitySkinPublishMessage } from "./community-helpers";
import { communityPublishFields } from "./community-publish-validation";
import type { CommunitySkinCategory } from "./community-skin-category-model";
import type { CommunitySkinClient } from "./community-skin-model";
import { useCommunityPublicationDraft } from "./use-community-publication-draft";

export interface CommunitySkinPublicationOptions {
  client: Pick<CommunitySkinClient, "publish">;
  library: Pick<CustomSkinLibraryClient, "load">;
  onPublished: () => Promise<void>;
}

/** 本机键盘设计的社区发布状态；调用方持有表单事件、关闭和成功后的导航。 */
export function useCommunitySkinPublication({
  client,
  library,
  onPublished,
}: CommunitySkinPublicationOptions) {
  const [saved, setSaved] = useState<SavedTouchKeyboardSkin[]>([]);
  const [selectedId, setSelectedId] = useState("");
  const {
    name,
    description,
    agreed,
    publicationId,
    setName,
    onNameChange,
    onDescriptionChange,
    onAgreedChange,
    resetPublication,
  } = useCommunityPublicationDraft();
  const [category, setCategory] = useState<CommunitySkinCategory>("other");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  // 错误文案会合并错误码，单独保留登录状态，界面才能提供可执行的登录入口。
  const [signInRequired, setSignInRequired] = useState(false);
  const {
    busy: actionBusy,
    generation: clientGeneration,
    running: actionRunning,
    run: runAsyncAction,
  } = useAsyncActionRunner(setError, undefined, client, library);
  const busy = loading || actionBusy;

  useEffect(() => {
    const generation = clientGeneration.current;
    setLoading(true);
    void library
      .load()
      .then((items) => {
        if (generation !== clientGeneration.current) return;
        setSaved(items);
        const first = items[0];
        if (first) {
          setSelectedId(first.id);
          setName(first.name);
        }
      })
      .catch((loadError) => {
        if (generation !== clientGeneration.current) return;
        setError(communitySkinPublishMessage(loadError));
        setSignInRequired(communityNeedsSignIn(loadError));
      })
      .finally(() => {
        if (generation === clientGeneration.current) setLoading(false);
      });
  }, [client, clientGeneration, library]);

  const selected = saved.find((item) => item.id === selectedId) ?? null;
  const submit = async () => {
    if (busy || actionRunning.current || !selected) return;
    const { normalizedName, normalizedDescription, nameValid, descriptionValid } =
      communityPublishFields(name, description);
    if (!nameValid || !descriptionValid || !agreed) {
      setError("请填写有效名称和说明，并确认拥有公开发布所需的素材权利。");
      return;
    }
    setSignInRequired(false);
    await runAsyncAction(
      async (isCurrent) => {
        await client.publish(
          publicationId,
          normalizedName,
          normalizedDescription,
          selected.design,
          category,
        );
        if (!isCurrent()) return;
        await onPublished();
      },
      {
        formatError: communitySkinPublishMessage,
        onError: (publishError) => setSignInRequired(communityNeedsSignIn(publishError)),
      },
    );
  };

  const selectDesign = (nextId: string) => {
    const item = saved.find((value) => value.id === nextId);
    setSelectedId(nextId);
    resetPublication();
    if (item) setName(item.name);
  };
  const changeCategory = (next: CommunitySkinCategory) => {
    // 分类也是这次发布的内容，换了分类就是另一次发布，不能沿用上一次的发布 id。
    resetPublication();
    setCategory(next);
  };

  return {
    saved,
    selectedId,
    selected,
    name,
    description,
    agreed,
    category,
    busy,
    error,
    signInRequired,
    onNameChange,
    onDescriptionChange,
    onAgreedChange,
    selectDesign,
    changeCategory,
    submit,
  };
}
