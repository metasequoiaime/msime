import { useEffect, useState } from "react";
import { useConfirm } from "../core/confirm";
import { useLatestRef } from "../core/use-latest-ref";
import { usePanelAction } from "./use-panel-action";
import { cloudDictionaryErrorMessage } from "./cloud-dictionary-errors";
import { candidateMutationCode, cloudResponseInteger } from "./cloud-response";
import type {
  CloudDictionaryKind,
  CloudCandidateKind,
  CloudRankingMode,
  CloudCandidate,
  CloudFixedPosition,
  CloudDictionaryPanelClient,
} from "./panel-client";

/** 管理云端候选查询、调频和固定位置，并隔离失效宿主的确认与回复。 */
export function useCloudCandidatesPanel(client: CloudDictionaryPanelClient) {
  const { confirm, dismiss, confirmation } = useConfirm();
  const [kind, setKind] = useState<CloudDictionaryKind>("pinyin");
  const [text, setText] = useState("");
  const [scheme, setScheme] = useState("pinyin");
  const [profile, setProfile] = useState("xiaohe");
  const [jianpin, setJianpin] = useState(false);
  const [mode, setMode] = useState<CloudRankingMode>("pin");
  const [step, setStep] = useState(1);
  const [trigger, setTrigger] = useState(1);
  const [forceTop, setForceTop] = useState(false);
  const [position, setPosition] = useState(1);
  const [query, setQuery] = useState<{
    text: string;
    kind: CloudCandidateKind;
    scheme: string;
    profile: string;
    limit: number;
  } | null>(null);
  const [candidates, setCandidates] = useState<CloudCandidate[]>([]);
  const [positions, setPositions] = useState<CloudFixedPosition[]>([]);
  const [context, setContext] = useState("");
  const [revision, setRevision] = useState(0);
  const [notice, setNotice] = useState("仅在点击查询时发送编码；修改只保存到当前账号");
  const { busy, busyRef, revisionRef, run, invalidate, isCurrent } = usePanelAction(
    setNotice,
    cloudDictionaryErrorMessage,
  );
  const textRef = useLatestRef(text);

  useEffect(() => {
    invalidate();
    dismiss();
    setCandidates([]);
    setPositions([]);
    setQuery(null);
    setContext("");
    setRevision(0);
    return () => {
      invalidate();
    };
  }, [client, dismiss, invalidate]);

  async function load(
    current: number,
    nextQuery: {
      text: string;
      kind: CloudCandidateKind;
      scheme: string;
      profile: string;
      limit: number;
    },
  ) {
    let result;
    try {
      result = await client.request({ operation: "candidates", ...nextQuery });
    } catch (error) {
      if (isCurrent(current)) {
        setCandidates([]);
        setPositions([]);
        setQuery(null);
        setContext("");
        setRevision(0);
      }
      throw error;
    }
    if (!isCurrent(current)) return;
    const nextCandidates = Array.isArray(result.candidates)
      ? result.candidates.filter(
          (candidate) =>
            candidate && typeof candidate.code === "string" && typeof candidate.word === "string",
        )
      : [];
    // 候选到达后立即显示；固定位置是次请求，不能拖延可见结果，
    // 否则触控宿主上快速返回的查询也会看起来像超时。
    setCandidates(nextCandidates);
    setContext(typeof result.context === "string" ? result.context : "");
    setRevision(cloudResponseInteger(result.revision) ? result.revision : 0);
    setPositions([]);
    setQuery(nextQuery);
    if (nextQuery.kind !== "quick" && typeof result.context === "string" && result.context) {
      const fixed = await client.request({
        operation: "fixed_positions",
        context: result.context,
        offset: 0,
      });
      if (!isCurrent(current)) return;
      const nextPositions = Array.isArray(fixed.positions)
        ? fixed.positions.filter(
            (item) => item && typeof item.code === "string" && typeof item.word === "string",
          )
        : [];
      setPositions(nextPositions);
    }
  }

  function queryCandidates() {
    const value = textRef.current.trim();
    if (!value) {
      setNotice("请输入编码");
      return;
    }
    const nextQuery = {
      text: value,
      kind: jianpin && kind === "pinyin" ? ("jianpin" as const) : kind,
      scheme,
      profile,
      limit: 100,
    };
    return run(async (current) => {
      await load(current, nextQuery);
      if (isCurrent(current)) setNotice("云端候选已刷新");
    }, "无法查询云端候选，请确认账号已登录");
  }

  async function reload(current: number, message: string) {
    if (!query || !isCurrent(current)) return;
    try {
      await load(current, query);
      if (isCurrent(current)) setNotice(message);
    } catch {
      if (isCurrent(current)) setNotice(`${message}，但候选刷新失败，请重新查询`);
    }
  }

  async function rank(candidate: CloudCandidate) {
    if (!query || busyRef.current) return;
    const current = revisionRef.current;
    if (!(await confirm({ title: "调整排序", message: "调整此云端候选的排序。" }))) return;
    if (!isCurrent(current)) return;
    return run(async (current) => {
      const result = await client.request({
        operation: "rank",
        ...query,
        code: candidateMutationCode(candidate),
        word: candidate.word,
        revision,
        mode,
        linear_step: step,
        trigger_count: trigger,
        force_top: forceTop,
      });
      await reload(
        current,
        result.changed === true
          ? "云端排序已更新"
          : `已记录本次选择，本次未调整排序；累计 ${result.selection_count ?? 0} 次`,
      );
    }, "云端调频失败，请刷新后重试");
  }

  async function remove(candidate: CloudCandidate) {
    if (!query || busyRef.current || (kind !== "english" && Array.from(candidate.word).length <= 1))
      return;
    const current = revisionRef.current;
    const confirmed = await confirm({
      title: "删除云端候选",
      message: `“${candidate.word}”会从云端候选中删除。`,
      confirmLabel: "删除",
      danger: true,
    });
    if (!confirmed || !isCurrent(current)) return;
    return run(async (current) => {
      await client.request({
        operation: "remove_candidate",
        ...query,
        code: candidateMutationCode(candidate),
        word: candidate.word,
        revision,
      });
      await reload(current, "云端候选已删除");
    }, "删除云端候选失败，请刷新后重试");
  }

  async function setFixed(candidate: CloudCandidate, nextPosition: number | null) {
    if (!query || !context || busyRef.current) return;
    const current = revisionRef.current;
    const confirmed = await confirm(
      nextPosition === null
        ? { title: "取消固定位置", message: `“${candidate.word}”将不再固定在某一位。` }
        : { title: "固定位置", message: `“${candidate.word}”将固定到第 ${nextPosition} 位。` },
    );
    if (!confirmed || !isCurrent(current)) return;
    return run(async (current) => {
      await client.request({
        operation: "set_fixed_position",
        context,
        code: candidateMutationCode(candidate),
        word: candidate.word,
        position: nextPosition,
        revision,
      });
      await reload(current, nextPosition === null ? "固定位置已取消" : "固定位置已更新");
    }, "固定位置更新失败，请刷新后重试");
  }

  async function unfix(item: CloudFixedPosition) {
    if (!query || busyRef.current) return;
    const current = revisionRef.current;
    const confirmed = await confirm({
      title: "取消固定位置",
      message: `“${item.word}”将不再固定在某一位。`,
    });
    if (!confirmed || !isCurrent(current)) return;
    return run(async (current) => {
      await client.request({
        operation: "set_fixed_position",
        context: item.context,
        code: item.code,
        word: item.word,
        position: null,
        revision,
      });
      await reload(current, "固定位置已取消");
    }, "固定位置更新失败，请刷新后重试");
  }

  function changeKind(next: CloudDictionaryKind) {
    if (busyRef.current || next === kind) return;
    setKind(next);
    setJianpin(false);
    setCandidates([]);
    setPositions([]);
    setQuery(null);
    setContext("");
  }

  return {
    confirmation,
    kind,
    text,
    setText,
    scheme,
    setScheme,
    profile,
    setProfile,
    jianpin,
    setJianpin,
    mode,
    setMode,
    step,
    setStep,
    trigger,
    setTrigger,
    forceTop,
    setForceTop,
    position,
    setPosition,
    query,
    candidates,
    positions,
    context,
    revision,
    notice,
    busy,
    queryCandidates,
    rank,
    remove,
    setFixed,
    unfix,
    changeKind,
  };
}

export type CloudCandidatesPanelState = ReturnType<typeof useCloudCandidatesPanel>;
