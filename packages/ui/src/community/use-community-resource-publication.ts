import { useState } from "react";
import { randomUuid } from "../core/random-id";
import { useAsyncActionRunner } from "../core/use-async-action";
import { resourceMessage } from "./community-helpers";
import { communityPublishFields } from "./community-publish-validation";
import type {
  CommunityResourceClient,
  CommunityResourceKind,
  CommunityResource,
  CommunitySharedWord,
} from "./community-resource-model";

export interface CommunityResourcePublicationOptions {
  client: Pick<CommunityResourceClient, "publish">;
  kind: CommunityResourceKind;
  existing?: CommunityResource;
  onPublished: () => Promise<void>;
}

/** 词库与回复模板的编辑、校验和发布；关闭与列表刷新由调用方持有。 */
export function useCommunityResourcePublication({
  client,
  kind,
  existing,
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
    setEntries((current) => [...current, value]);
    setCode("");
    setWord("");
    setError("");
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
      { formatError: resourceMessage },
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
