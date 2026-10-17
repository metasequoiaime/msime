import { useState } from "react";
import { randomUuid } from "../core/random-id";
import { useAsyncActionRunner } from "../core/use-async-action";
import { resourceMessage } from "./community-helpers";
import { communityPublishFields } from "./community-publish-validation";
import type {
  CommunityLocalDictionaryClient,
  CommunityResourceClient,
  CommunityResourceKind,
  CommunityResource,
  CommunitySharedWord,
} from "./community-resource-model";

export interface CommunityResourcePublicationOptions {
  client: Pick<CommunityResourceClient, "publish">;
  kind: CommunityResourceKind;
  existing?: CommunityResource;
  /** 宿主有命名词库时用来核对词条是否来自用户导入的搜狗细胞词库，见 [`thirdPartyEntryMessage`]。 */
  localDictionary?: Pick<CommunityLocalDictionaryClient, "thirdPartyMatches">;
  onPublished: () => Promise<void>;
}

/** 发布词库的编辑器里一直显示的说明。 */
export const COMMUNITY_DICTIONARY_SOURCE_NOTE =
  "只发布你自己整理的词条。从搜狗细胞词库等第三方词库导入的词不能发布：它们的整理成果属于原作者，公开分享可能侵犯原作者的权利。";

/** 词条来自用户导入的搜狗细胞词库时拒绝的原因。社区是这些词唯一会被「向公众提供」的出口，所以在这里拦下；依据见 `.agents/notes/implemented/feature/2026-10-17-scel-community-publish-guard.md`。 */
export function thirdPartyEntryMessage(word: string): string {
  return `“${word}”来自你导入的搜狗细胞词库，不能发布到社区：第三方词库的整理成果属于原作者，公开分享可能侵犯他们的权利。`;
}

/** 发布时核对出第三方词库的词条：错误信息直接给用户看，不走社区接口的错误码翻译。 */
class ThirdPartyEntryError extends Error {}

/** 词库与回复模板的编辑、校验和发布；关闭与列表刷新由调用方持有。 */
export function useCommunityResourcePublication({
  client,
  kind,
  existing,
  localDictionary,
  onPublished,
}: CommunityResourcePublicationOptions) {
  const [id] = useState(existing?.id ?? randomUuid());
  const [name, setName] = useState(existing?.name ?? "");
  const [description, setDescription] = useState(existing?.description ?? "");
  const [prompt, setPrompt] = useState(existing?.content.prompt ?? "");
  const [entries, setEntries] = useState<CommunitySharedWord[]>(existing?.content.entries ?? []);
  const [entryKind, setEntryKind] = useState<CommunitySharedWord["kind"]>("pinyin");
  const [code, setCode] = useState("");
  const [word, setWord] = useState("");
  const [weight, setWeight] = useState("100000");
  const [agreed, setAgreed] = useState(false);
  const [error, setError] = useState("");
  const {
    busy,
    running: actionRunning,
    run: runAsyncAction,
  } = useAsyncActionRunner(setError, undefined, client);
  /** 第一个来自搜狗细胞词库的词条；宿主不能核对时为 `undefined`。核对本身失败时不拦：命名词库读不出来不该让用户发布不了自己整理的词。 */
  const firstThirdPartyEntry = async (candidates: CommunitySharedWord[]) => {
    const check = localDictionary?.thirdPartyMatches;
    if (!check || candidates.length === 0) return undefined;
    let matches: number[];
    try {
      matches = await check(candidates);
    } catch {
      return undefined;
    }
    const first = matches.length > 0 ? Math.min(...matches) : -1;
    return first >= 0 ? candidates[first] : undefined;
  };
  const acceptEntry = (value: CommunitySharedWord) => {
    // 核对是异步的，连点两次可能各自通过检查；这里再按同一个词去一次重。
    setEntries((current) =>
      current.some(
        (item) => item.kind === value.kind && item.code === value.code && item.word === value.word,
      )
        ? current
        : [...current, value],
    );
    setCode("");
    setWord("");
    setError("");
  };
  /** 宿主不能核对时同步加入；能核对时先问宿主，来自细胞词库的词不加入并说明原因。 */
  const addEntry = () => {
    const value = { kind: entryKind, code: code.trim(), word, weight: Number(weight) };
    if (
      !value.code ||
      !value.word ||
      !Number.isSafeInteger(value.weight) ||
      value.weight < 0 ||
      entries.some(
        (item) => item.kind === value.kind && item.code === value.code && item.word === value.word,
      ) ||
      entries.length >= 128
    ) {
      setError("词条不能为空、不能重复，权重必须为非负整数，且最多 128 条。");
      return;
    }
    if (!localDictionary?.thirdPartyMatches) {
      acceptEntry(value);
      return;
    }
    return firstThirdPartyEntry([value]).then((thirdParty) => {
      if (thirdParty) setError(thirdPartyEntryMessage(thirdParty.word));
      else acceptEntry(value);
    });
  };
  const submit = async () => {
    if (busy || actionRunning.current) return;
    const { normalizedName, normalizedDescription, nameValid, descriptionValid } =
      communityPublishFields(name, description);
    const valid =
      nameValid &&
      descriptionValid &&
      (kind === "reply"
        ? prompt.trim().length > 0 && [...prompt].length <= 2000
        : entries.length > 0) &&
      (existing || agreed);
    if (!valid) {
      setError(
        existing ? "请填写有效的作品信息。" : "请填写有效内容并确认拥有公开发布所需的权利。",
      );
      return;
    }
    await runAsyncAction(
      async (isCurrent) => {
        // 发布前再核对一次整份：更新旧作品时带进来的词条，以及添加之后才导入的细胞词库，都要在这里拦下。放在动作里面，防重和忙碌状态照旧。
        if (kind === "dictionary" && localDictionary?.thirdPartyMatches) {
          const thirdParty = await firstThirdPartyEntry(entries);
          if (thirdParty) throw new ThirdPartyEntryError(thirdPartyEntryMessage(thirdParty.word));
        }
        await client.publish(
          id,
          kind,
          normalizedName,
          normalizedDescription,
          kind === "reply" ? { prompt } : { entries },
          existing?.revision ?? 0,
        );
        if (!isCurrent()) return;
        await onPublished();
      },
      {
        formatError: (error) =>
          error instanceof ThirdPartyEntryError ? error.message : resourceMessage(error),
      },
    );
  };
  const removeEntry = (index: number) => {
    setEntries(entries.filter((_, current) => current !== index));
  };

  return {
    name,
    description,
    prompt,
    entries,
    entryKind,
    code,
    word,
    weight,
    agreed,
    error,
    busy,
    onNameChange: setName,
    onDescriptionChange: setDescription,
    onPromptChange: setPrompt,
    onCodeChange: setCode,
    onWordChange: setWord,
    onWeightChange: setWeight,
    onAgreedChange: setAgreed,
    onEntryKindChange: (value: string) => setEntryKind(value as CommunitySharedWord["kind"]),
    addEntry,
    removeEntry,
    submit,
  };
}
