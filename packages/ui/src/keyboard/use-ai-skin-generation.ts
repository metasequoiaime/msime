import { useEffect, useRef, useState } from "react";
import { errorCode } from "../core/error-code";
import { randomRequestId } from "../core/random-id";
import { trackAsyncSubscription } from "../core/async-subscription";
import { useAsyncActionRunner } from "../core/use-async-action";
import { useAsyncGeneration } from "../settings/use-async-generation";
import type { CommunitySkinClient } from "../community/community-skin-model";
import type { CommunitySkinCategory } from "../community/community-skin-category-model";
import { aiSkinMessage, libraryError } from "./touch-keyboard-skin-errors";
import { createAiSkinPrompt } from "./touch-keyboard-skin-ai";
import { boundTouchSkinArtwork } from "./touch-skin-images";
import {
  updateSavedTouchKeyboardSkinName,
  type AiSkinClient,
  type AiSkinProposal,
  type CustomSkinLibraryClient,
  type SavedTouchKeyboardSkin,
} from "./touch-keyboard-skin-design";

/** 服务的对象身份共同界定抽卡会话；展示回调不参与会话归属。 */
export interface UseAiSkinGenerationOptions {
  client: AiSkinClient;
  library: CustomSkinLibraryClient;
  communitySkins?: CommunitySkinClient;
}

/** 抽卡、插画准备、保存与社区发布的会话状态，独立于对话框与全屏布局。 */
export function useAiSkinGeneration({
  client,
  library,
  communitySkins,
}: UseAiSkinGenerationOptions) {
  const [proposals, setProposals] = useState<AiSkinProposal[]>([]);
  const [completed, setCompleted] = useState(0);
  const [message, setMessage] = useState("");
  const [requestId, setRequestId] = useState("");
  const requestRef = useRef("");
  // 名称可在不同批次重复；保存记录必须属于具体提案，不能误用同名旧设计。
  const savedRef = useRef(new Map<AiSkinProposal, SavedTouchKeyboardSkin>());
  const [saved, setSaved] = useState<ReadonlyMap<AiSkinProposal, SavedTouchKeyboardSkin>>(
    savedRef.current,
  );
  const [publishing, setPublishing] = useState<SavedTouchKeyboardSkin | null>(null);
  const [publishDescription, setPublishDescription] = useState("");
  const [publishAgreed, setPublishAgreed] = useState(false);
  const [publishCategory, setPublishCategory] = useState<CommunitySkinCategory>("other");
  const {
    busy,
    mounted,
    run: runGeneration,
    invalidate: invalidateGeneration,
  } = useAsyncActionRunner(setMessage, undefined, client, library, communitySkins);
  const {
    busy: saveBusy,
    generation: saveGeneration,
    run: runSave,
  } = useAsyncActionRunner(setMessage, undefined, client, library, communitySkins);
  const {
    busy: publishBusy,
    running: publishRunning,
    run: runPublish,
  } = useAsyncActionRunner(setMessage, undefined, client, library, communitySkins);

  useEffect(() => {
    setProposals([]);
    setCompleted(0);
    setMessage("");
    setRequestId("");
    savedRef.current = new Map();
    setSaved(savedRef.current);
    setPublishing(null);
    setPublishDescription("");
    setPublishAgreed(false);
    setPublishCategory("other");
    return () => {
      const id = requestRef.current;
      requestRef.current = "";
      if (id) void client.cancel(id).catch(() => undefined);
    };
  }, [client, library, communitySkins]);

  const progressGeneration = useAsyncGeneration(client, library, communitySkins, requestId);
  useEffect(() => {
    const generation = progressGeneration.current;
    if (!client.onProgress) return;
    return trackAsyncSubscription(
      client.onProgress((progress) => {
        if (
          requestRef.current &&
          generation === progressGeneration.current &&
          progress.requestId === requestRef.current
        )
          setCompleted(progress.completed);
      }),
      () => generation === progressGeneration.current,
    );
  }, [client, library, communitySkins, progressGeneration, requestId]);

  const cancelGeneration = () => {
    const id = requestRef.current;
    if (!id) return;
    // 本地先失效并解锁，迟到的结果、错误与收尾都不能影响下一次抽卡。
    requestRef.current = "";
    invalidateGeneration();
    setRequestId("");
    setMessage("已取消生成");
    void client.cancel(id).catch(() => undefined);
  };

  const generate = () =>
    runGeneration(
      async (isCurrent) => {
        const id = randomRequestId("ai-skin");
        requestRef.current = id;
        setRequestId(id);
        setCompleted(0);
        try {
          const result = await client.generate(id, createAiSkinPrompt());
          if (!isCurrent() || requestRef.current !== id) return;
          const prepared = await Promise.all(
            result.map(async (proposal) => ({
              ...proposal,
              design: {
                ...proposal.design,
                photo: await boundTouchSkinArtwork(proposal.artwork),
                photoShade: 0.08,
                photoPosition: 0.5,
                keyOpacity: 0.92,
                pattern: 0 as const,
              },
            })),
          );
          if (isCurrent() && requestRef.current === id) setProposals(prepared);
        } finally {
          if (isCurrent() && requestRef.current === id) {
            requestRef.current = "";
            setRequestId("");
          }
        }
      },
      {
        formatError: aiSkinMessage,
        ignoreError: (error) => errorCode(error) === "ai_skin_cancelled",
      },
    );

  const save = async (proposal: AiSkinProposal): Promise<SavedTouchKeyboardSkin | null> => {
    const existing = savedRef.current.get(proposal);
    if (existing) return existing;
    let savedItem: SavedTouchKeyboardSkin | null = null;
    await runSave(
      async (isCurrent) => {
        const current = await library.load();
        if (!isCurrent()) return;
        if (current.length >= 12) throw { code: "custom_skin_full" };
        let name = proposal.name;
        let suffix = 2;
        while (current.some((item) => item.name === name)) {
          name = `${proposal.name.slice(0, 26)} ${suffix}`;
          suffix += 1;
        }
        const next = await library.mutate({ operation: "create", name, design: proposal.design });
        if (!isCurrent()) return;
        const item = next.find((value) => value.name === name);
        if (!item) throw new Error("skin was not saved");
        savedRef.current = new Map(savedRef.current).set(proposal, item);
        setSaved(savedRef.current);
        setMessage("已保存到“我的皮肤”。");
        savedItem = item;
      },
      { formatError: libraryError },
    );
    return savedItem;
  };

  const beginPublish = async (proposal: AiSkinProposal) => {
    if (!communitySkins || publishRunning.current) return;
    const generation = saveGeneration.current;
    const item = await save(proposal);
    if (
      !item ||
      !mounted.current ||
      generation !== saveGeneration.current ||
      publishRunning.current
    )
      return;
    setPublishing(item);
    setPublishDescription(proposal.description);
    setPublishAgreed(false);
    setPublishCategory("other");
  };

  const publish = () => {
    if (
      !publishing ||
      !communitySkins ||
      !publishAgreed ||
      publishRunning.current ||
      !mounted.current
    )
      return;
    const name = publishing.name.trim();
    const description = publishDescription.trim();
    if (!name || name.length > 32 || description.length > 280) {
      setMessage("请填写有效的名称和设计说明。");
      return;
    }
    return runPublish(
      async (isCurrent) => {
        await communitySkins.publish(
          publishing.id,
          name,
          description,
          publishing.design,
          publishCategory,
        );
        if (!isCurrent()) return;
        setPublishing(null);
        setPublishDescription("");
        setPublishAgreed(false);
        setPublishCategory("other");
        setMessage("已发布到社区。");
      },
      {
        formatError: (error) => {
          const code = errorCode(error);
          return code ? `发布失败：${code}` : "暂时无法发布皮肤，请稍后重试。";
        },
      },
    );
  };

  const changePublishName = (name: string) =>
    setPublishing((current) =>
      current ? updateSavedTouchKeyboardSkinName(current, name) : current,
    );
  const closePublish = () => {
    if (!publishRunning.current) setPublishing(null);
  };

  return {
    proposals,
    busy,
    completed,
    message,
    saved,
    saveBusy,
    publishing,
    publishDescription,
    publishAgreed,
    publishCategory,
    publishBusy,
    generate,
    cancelGeneration,
    save,
    beginPublish,
    publish,
    changePublishName,
    setPublishDescription,
    setPublishAgreed,
    setPublishCategory,
    closePublish,
  };
}

export type AiSkinGenerationState = ReturnType<typeof useAiSkinGeneration>;
