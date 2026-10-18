import { useEffect, useRef, useState, type PointerEvent } from "react";
import { isComposingKeyEvent } from "../core/ime-key-event";
import { focusAndRevealNearest } from "../core/focus-and-reveal-nearest";
import { clamp } from "../core/number";
import { readOptionalStorage, writeOptionalStorage } from "../core/optional-storage";
import { useLatestRef } from "../core/use-latest-ref";
import { useAsyncGeneration } from "../settings/use-async-generation";
import { useResourcePacks, resourcePackStatus } from "../settings/resource-packs";
import { usePanelDrag } from "./use-panel-drag";
import { normalizeHandwritingCandidates } from "./handwriting";
import {
  appendPointerSamples,
  MAX_HANDWRITING_STROKES,
  WINDOWS_HANDWRITING_STROKES,
  type Point,
} from "./handwriting-input";
import type { InkStroke, PanelClient } from "./panel-client";
import type { PanelActivationMode } from "./panel-activation-buttons";

export function useHandwritingPanel({
  client,
  platform,
}: {
  client: PanelClient;
  platform?: string;
}) {
  const maxStrokes = platform === "windows" ? WINDOWS_HANDWRITING_STROKES : MAX_HANDWRITING_STROKES;
  // 手写模型按需下载：宿主列出手写模型时，第一次打开面板就开始下载，下载完成前不调用识别器，笔画保留，装好后自动识别当前笔画。宿主没列出它（随包带着，或 Windows Ink 有中文识别器）时不下载也不等待。
  const packs = useResourcePacks(client.resourcePacks);
  const handwritingPack = resourcePackStatus(packs, "handwriting");
  // 只有确定缺少模型时才暂停识别；读不到列表时照常识别。已过期（文件字节与锁文件不一致）的资源包会由 ensure 重新下载，期间识别照常尝试，宿主可能还有别的模型可用。
  const modelMissing = handwritingPack?.state === "missing";
  const modelMissingRef = useLatestRef(modelMissing);
  const waitingForModel = useRef(false);
  const ensuredModel = useRef(false);
  useEffect(() => {
    if (ensuredModel.current || !handwritingPack) return;
    ensuredModel.current = true;
    packs.ensure("handwriting");
  }, [handwritingPack, packs]);
  const [activationMode, setActivationMode] = useState<PanelActivationMode>(() => {
    return readOptionalStorage("msime.handwriting.activation") === "input" ? "input" : "copy";
  });
  const effectiveMode =
    activationMode === "copy" && !client.copyHandwritingCandidate
      ? "input"
      : activationMode === "input" && !client.submitHandwritingCandidate
        ? "copy"
        : activationMode;
  function selectActivation(mode: PanelActivationMode) {
    if (!recognitionQueue.current.active || closingRef.current) return;
    setActivationMode(mode);
    writeOptionalStorage("msime.handwriting.activation", mode);
  }

  const [strokes, setStrokes] = useState<InkStroke[]>([]);
  const [redoStrokes, setRedoStrokes] = useState<InkStroke[]>([]);
  const [drawing, setDrawing] = useState<Point[]>([]);
  const [candidates, setCandidates] = useState<string[]>([]);
  const [notice, setNotice] = useState("请在左侧书写，松开鼠标后自动识别");
  const drag = usePanelDrag(client, () => setNotice("无法移动窗口，请重试。"));
  const recognitionRevision = useAsyncGeneration();
  const [recognizing, setRecognizing] = useState(false);
  const recognitionQueue = useRef<{
    active: boolean;
    running: boolean;
    pending: { revision: number; strokes: InkStroke[] } | null;
  }>({ active: true, running: false, pending: null });
  const submissionRevision = useAsyncGeneration();
  const closingRef = useRef(false);
  const [closing, setClosing] = useState(false);
  const submittingRef = useRef(false);
  const [submitting, setSubmitting] = useState(false);
  const activeStroke = useRef<{ pointerId: number; canvas: SVGSVGElement; points: Point[] } | null>(
    null,
  );
  function releaseStroke() {
    const active = activeStroke.current;
    activeStroke.current = null;
    if (active?.canvas.hasPointerCapture?.(active.pointerId))
      active.canvas.releasePointerCapture(active.pointerId);
  }
  useEffect(() => {
    const queue = {
      active: true,
      running: false,
      pending: null,
    } as typeof recognitionQueue.current;
    recognitionQueue.current = queue;
    setRecognizing(false);
    closingRef.current = false;
    setClosing(false);
    submittingRef.current = false;
    setSubmitting(false);
    return () => {
      queue.active = false;
      queue.pending = null;
      recognitionRevision.current++;
      submissionRevision.current++;
      submittingRef.current = false;
      releaseStroke();
    };
  }, [client]);
  async function closeHandwriting() {
    const queue = recognitionQueue.current;
    if (!queue.active || closingRef.current) return;
    if (activeStroke.current) {
      setNotice("请完成当前笔画后关闭手写识别板");
      return;
    }
    closingRef.current = true;
    setClosing(true);
    try {
      await client.close();
      if (queue.active && recognitionQueue.current === queue) {
        queue.active = false;
        queue.pending = null;
        recognitionRevision.current++;
        submissionRevision.current++;
        releaseStroke();
      }
    } catch {
      if (queue.active && recognitionQueue.current === queue) {
        setNotice("无法关闭手写识别板，请重试");
      }
    } finally {
      if (queue.active && recognitionQueue.current === queue) {
        closingRef.current = false;
        setClosing(false);
      }
    }
  }
  function recognize(nextStrokes: InkStroke[]) {
    if (!recognitionQueue.current.active || closingRef.current) return;
    const revision = ++recognitionRevision.current;
    setCandidates([]);
    setNotice("正在识别…");
    const recognizeInk = client.recognizeHandwriting;
    if (!recognizeInk) {
      setNotice("识别结果需由宿主提供");
      return;
    }
    if (modelMissingRef.current) {
      waitingForModel.current = true;
      setNotice("手写模型下载完成后自动识别");
      return;
    }
    const queue = recognitionQueue.current;
    queue.pending = { revision, strokes: nextStrokes };
    setRecognizing(true);
    if (queue.running) return;
    queue.running = true;
    void (async () => {
      try {
        while (queue.active && queue === recognitionQueue.current && queue.pending) {
          const request = queue.pending;
          queue.pending = null;
          if (request.revision !== recognitionRevision.current) continue;
          try {
            const result = await recognizeInk({ language: "zh-CN", strokes: request.strokes });
            if (request.revision !== recognitionRevision.current) continue;
            const nextCandidates = normalizeHandwritingCandidates(result.candidates);
            setCandidates(nextCandidates);
            setNotice(
              nextCandidates.length
                ? "选择候选可复制或输入"
                : platform === "windows"
                  ? "未识别到内容，请确认已安装中文手写包"
                  : "未识别到内容，请重写",
            );
          } catch {
            if (request.revision === recognitionRevision.current) {
              setCandidates([]);
              setNotice("手写识别失败，请确认手写模型或识别服务可用，然后重新识别");
            }
          }
        }
      } finally {
        queue.running = false;
        if (queue.active && queue === recognitionQueue.current) setRecognizing(false);
      }
    })();
  }
  function retryRecognition() {
    if (!recognitionQueue.current.active || closingRef.current) return;
    if (
      !strokes.length ||
      activeStroke.current ||
      recognitionQueue.current.running ||
      submittingRef.current
    )
      return;
    recognize(strokes);
  }
  // 模型装好后，对等待中的当前笔画补一次识别；正在书写时交给笔画结束时的识别。
  useEffect(() => {
    if (modelMissing || !waitingForModel.current) return;
    waitingForModel.current = false;
    if (strokes.length && !activeStroke.current) recognize(strokes);
  }, [modelMissing]);
  function start(event: PointerEvent<SVGSVGElement>) {
    if (
      !recognitionQueue.current.active ||
      closingRef.current ||
      activeStroke.current ||
      event.isPrimary === false ||
      (event.button !== undefined && event.button !== 0)
    )
      return;
    if (strokes.length >= maxStrokes) {
      setNotice("笔画已达上限，请选择候选、撤销或重写");
      return;
    }
    event.currentTarget.focus({ preventScroll: true });
    recognitionRevision.current++;
    setCandidates([]);
    setNotice("书写中，松开后自动识别");
    const points = appendPointerSamples([], event);
    if (!points.length) return;
    activeStroke.current = { pointerId: event.pointerId, canvas: event.currentTarget, points };
    event.currentTarget.setPointerCapture?.(event.pointerId);
    setDrawing(points);
  }
  function move(event: PointerEvent<SVGSVGElement>) {
    const active = activeStroke.current;
    if (!active || active.pointerId !== event.pointerId) return;
    active.points = appendPointerSamples(active.points, event);
    setDrawing(active.points);
  }
  function recognizeRemaining(nextStrokes: InkStroke[]) {
    setCandidates([]);
    if (!nextStrokes.length) setNotice("请在左侧书写，松开鼠标后自动识别");
    else void recognize(nextStrokes);
  }
  function end(event: PointerEvent<SVGSVGElement>) {
    const active = activeStroke.current;
    if (!active || active.pointerId !== event.pointerId) return;
    active.points = appendPointerSamples(active.points, event, true);
    // 与 Windows 手写板一致：没有移动的点击不算笔画，也不能触发识别请求。
    const nextStrokes =
      active.points.length >= 2 ? [...strokes, { points: active.points }] : strokes;
    releaseStroke();
    setStrokes(nextStrokes);
    if (active.points.length >= 2) setRedoStrokes([]);
    setDrawing([]);
    recognizeRemaining(nextStrokes);
  }
  function cancel(event: PointerEvent<SVGSVGElement>) {
    if (!activeStroke.current || activeStroke.current.pointerId !== event.pointerId) return;
    recognitionRevision.current++;
    releaseStroke();
    setDrawing([]);
    recognizeRemaining(strokes);
  }
  function undo() {
    if (!recognitionQueue.current.active || closingRef.current) return;
    const wasDrawing = activeStroke.current !== null;
    recognitionRevision.current++;
    recognitionQueue.current.pending = null;
    releaseStroke();
    setDrawing([]);
    if (wasDrawing) {
      // 只取消未完成的笔画，已完成的笔画继续保留。
      recognizeRemaining(strokes);
      return;
    }
    if (!strokes.length) return;
    setRedoStrokes((current) => [...current, strokes[strokes.length - 1]]);
    const nextStrokes = strokes.slice(0, -1);
    setStrokes(nextStrokes);
    recognizeRemaining(nextStrokes);
  }
  function redo() {
    if (!recognitionQueue.current.active || closingRef.current) return;
    if (activeStroke.current || !redoStrokes.length || strokes.length >= maxStrokes) return;
    recognitionRevision.current++;
    const nextStrokes = [...strokes, redoStrokes[redoStrokes.length - 1]];
    setRedoStrokes((current) => current.slice(0, -1));
    setStrokes(nextStrokes);
    recognizeRemaining(nextStrokes);
  }
  function clear() {
    if (!recognitionQueue.current.active || closingRef.current) return;
    recognitionRevision.current++;
    recognitionQueue.current.pending = null;
    releaseStroke();
    setStrokes([]);
    setRedoStrokes([]);
    setDrawing([]);
    setCandidates([]);
    setNotice("请在左侧书写，松开鼠标后自动识别");
  }
  function editInk(event: import("react").KeyboardEvent<HTMLElement>) {
    if (event.defaultPrevented || isComposingKeyEvent(event) || event.altKey) return;
    const target = event.target as HTMLElement;
    if (target.closest("input, textarea, select, [contenteditable=true]")) return;
    const key = event.key.toLowerCase();
    if ((event.ctrlKey || event.metaKey) && (key === "z" || key === "y")) {
      event.preventDefault();
      if (key === "y" || event.shiftKey) redo();
      else undo();
    } else if (event.key === "Escape" && activeStroke.current) {
      event.preventDefault();
      undo();
    }
  }
  function navigateCandidates(event: import("react").KeyboardEvent<HTMLDivElement>) {
    if (
      event.defaultPrevented ||
      isComposingKeyEvent(event) ||
      event.ctrlKey ||
      event.metaKey ||
      event.altKey ||
      event.shiftKey ||
      closingRef.current ||
      submittingRef.current
    )
      return;
    if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"].includes(event.key))
      return;
    const buttons = Array.from(
      event.currentTarget.querySelectorAll<HTMLButtonElement>(
        ".handwriting-candidate-submit:not(:disabled)",
      ),
    );
    const index = buttons.indexOf(event.target as HTMLButtonElement);
    if (index < 0) return;
    // 候选网格有四列，确保不完整的末行也能到达。
    const next =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? buttons.length - 1
          : index +
            (event.key === "ArrowLeft"
              ? -1
              : event.key === "ArrowRight"
                ? 1
                : event.key === "ArrowUp"
                  ? -4
                  : 4);
    event.preventDefault();
    const target = buttons[clamp(next, 0, buttons.length - 1)];
    focusAndRevealNearest(target);
  }
  async function chooseCandidate(candidate: string, copyOnly = false) {
    if (
      !recognitionQueue.current.active ||
      closingRef.current ||
      submittingRef.current ||
      !candidates.includes(candidate)
    )
      return;
    const action = copyOnly ? client.copyHandwritingCandidate : client.submitHandwritingCandidate;
    if (!action) {
      setNotice(`已选择：${candidate}（等待宿主提交能力）`);
      return;
    }
    const revision = ++submissionRevision.current;
    const inkRevision = recognitionRevision.current;
    submittingRef.current = true;
    setSubmitting(true);
    setNotice(copyOnly ? "正在复制候选…" : "正在提交候选…");
    try {
      await action(candidate);
      if (revision === submissionRevision.current && inkRevision === recognitionRevision.current) {
        setNotice(copyOnly ? `已复制：${candidate}` : `已提交：${candidate}`);
      }
    } catch {
      if (revision === submissionRevision.current && inkRevision === recognitionRevision.current) {
        setNotice(
          copyOnly
            ? "复制失败，候选和笔画已保留，请重试"
            : "提交失败，可复制候选后手动粘贴，或重试",
        );
      }
    } finally {
      if (revision === submissionRevision.current) {
        submittingRef.current = false;
        setSubmitting(false);
      }
    }
  }

  const renderStrokes = [...strokes, ...(drawing.length ? [{ points: drawing }] : [])];
  return {
    packs,
    handwritingPack,
    effectiveMode,
    selectActivation,
    maxStrokes,
    strokes,
    redoStrokes,
    drawing,
    candidates,
    notice,
    drag,
    recognizing,
    closing,
    submitting,
    renderStrokes,
    closeHandwriting,
    start,
    move,
    end,
    cancel,
    undo,
    redo,
    clear,
    retryRecognition,
    editInk,
    navigateCandidates,
    chooseCandidate,
  };
}

export type HandwritingPanelState = ReturnType<typeof useHandwritingPanel>;
