import { useEffect, useRef, useState } from "react";
import type { TypingStatisticsClient, TypingStatisticsStatus } from "./typing-statistics-model";
import type { TypingSummary } from "./typing-summary";
import { recentDays } from "./typing-statistics-helpers";
import { useMountedRef } from "./use-mounted-ref";
import { useAsyncGeneration } from "./use-async-generation";
import { subscribeToWindowResume } from "../core/window-focus";
import { subscribeToInterval } from "../core/use-interval";

const overviewPollMs = 5_000;
const overviewMinIntervalMs = 1_000;
const overviewStaleMs = 15_000;

/** 管理统计与摘要的读取、写入和刷新；外观标识变化时保留原来的订阅重建时机。 */
export function useTypingStatisticsState(client: TypingStatisticsClient, look?: string) {
  const [status, setStatus] = useState<TypingStatisticsStatus>();
  const [today, setToday] = useState(() => recentDays(1)[0]);
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const requestRef = useRef<Promise<TypingStatisticsStatus> | null>(null);
  const requestStartedAtRef = useRef(0);
  const lastRequestAtRef = useRef(0);
  const statusSignatureRef = useRef("");
  const [summary, setSummary] = useState<TypingSummary>();
  const [summaryFailed, setSummaryFailed] = useState(false);
  const summaryRequestRef = useRef<Promise<TypingSummary> | null>(null);
  const summarySignatureRef = useRef("");
  // 当前显示的摘要所对应的本地日期：页面开着过了午夜，即使没有新统计也会重新请求，因为摘要里的每个时间窗都截止到今天。
  const summaryDayRef = useRef("");
  // 摘要请求进行中统计又发生变化时置位，让它完成后再请求一次。
  const summaryDirtyRef = useRef(true);
  const mounted = useMountedRef();
  const clientGeneration = useAsyncGeneration(client);

  function refreshDay() {
    const next = recentDays(1)[0];
    // 数据没有变化时也要推进日历；同一天保留原对象，避免轮询重画整页。
    setToday((current) => (current.key === next.key ? current : next));
  }

  useEffect(
    () => () => {
      requestRef.current = null;
    },
    [],
  );

  useEffect(() => {
    requestRef.current = null;
    requestStartedAtRef.current = 0;
    lastRequestAtRef.current = 0;
    summaryRequestRef.current = null;
    summaryDayRef.current = "";
    summaryDirtyRef.current = true;
    setBusy(false);
    setError("");
  }, [client]);

  /**
   * 读取统计后请求摘要：统计有变化（`changed`）时总是请求，否则只在上次应答属于更早的日期或失败时请求。摘要是刷新中开销大的那一半，所以没发现新数据的 5 秒轮询不重新计算它。
   */
  function refreshSummary(changed: boolean) {
    if (changed) summaryDirtyRef.current = true;
    const summarize = client.summary;
    if (!look || !summarize || !mounted.current || summaryRequestRef.current) return;
    const today = recentDays(1)[0].key;
    if (!summaryDirtyRef.current && summaryDayRef.current === today) return;
    summaryDirtyRef.current = false;
    const generation = clientGeneration.current;
    const request = summarize.call(client);
    summaryRequestRef.current = request;
    const current = () =>
      mounted.current &&
      clientGeneration.current === generation &&
      summaryRequestRef.current === request;
    void request
      .then(
        (next) => {
          if (!current()) return;
          summaryDayRef.current = today;
          setSummaryFailed(false);
          const signature = JSON.stringify(next);
          if (signature !== summarySignatureRef.current) {
            summarySignatureRef.current = signature;
            setSummary(next);
          }
        },
        () => {
          if (!current()) return;
          // 清掉日期，下一次轮询就会重试。
          summaryDayRef.current = "";
          setSummaryFailed(true);
        },
      )
      .finally(() => {
        if (summaryRequestRef.current !== request) return;
        summaryRequestRef.current = null;
        if (summaryDirtyRef.current) refreshSummary(false);
      });
  }

  async function update(operation: () => Promise<TypingStatisticsStatus>, overview = false) {
    if (!mounted.current || requestRef.current) return;
    refreshDay();
    const generation = clientGeneration.current;
    setBusy(true);
    setError("");
    const request = operation();
    requestRef.current = request;
    requestStartedAtRef.current = Date.now();
    if (overview) lastRequestAtRef.current = requestStartedAtRef.current;
    try {
      const next = await request;
      if (
        !mounted.current ||
        clientGeneration.current !== generation ||
        requestRef.current !== request
      )
        return;
      statusSignatureRef.current = JSON.stringify(next);
      setStatus(next);
      refreshSummary(true);
    } catch {
      if (
        mounted.current &&
        clientGeneration.current === generation &&
        requestRef.current === request
      )
        setError("无法读取或保存统计，请稍后重试。原有统计不会被自动重置。");
    } finally {
      if (requestRef.current === request) {
        requestRef.current = null;
        if (mounted.current && clientGeneration.current === generation) setBusy(false);
      }
    }
  }

  useEffect(() => {
    const generation = clientGeneration.current;
    const refreshWhenVisible = () => {
      if (!mounted.current || clientGeneration.current !== generation) return;
      if (document.visibilityState === "hidden") return;
      refreshDay();
      const now = Date.now();
      if (
        (requestRef.current && now - requestStartedAtRef.current <= overviewStaleMs) ||
        now - lastRequestAtRef.current < overviewMinIntervalMs
      )
        return;
      lastRequestAtRef.current = now;
      setError("");
      const request = client.load();
      requestRef.current = request;
      requestStartedAtRef.current = now;
      void request
        .then((next) => {
          if (clientGeneration.current !== generation || requestRef.current !== request) return;
          const signature = JSON.stringify(next);
          const changed = signature !== statusSignatureRef.current;
          if (changed) {
            statusSignatureRef.current = signature;
            setStatus(next);
          }
          refreshSummary(changed);
        })
        .catch(() => {
          if (clientGeneration.current === generation && requestRef.current === request)
            setError("无法读取或保存统计，请稍后重试。原有统计不会被自动重置。");
        })
        .finally(() => {
          if (requestRef.current === request) {
            requestRef.current = null;
            if (mounted.current && clientGeneration.current === generation) setBusy(false);
          }
        });
    };
    refreshWhenVisible();
    const stopResume = subscribeToWindowResume(refreshWhenVisible);
    const stopPoll = subscribeToInterval(refreshWhenVisible, overviewPollMs);
    return () => {
      stopPoll();
      stopResume();
    };
  }, [client, clientGeneration, look]);

  async function openDataDirectory() {
    const openDirectory = client.openDirectory;
    if (!openDirectory) return;
    setError("");
    try {
      await openDirectory();
    } catch {
      setError("无法打开数据目录，可能是文件管理器不可用。");
    }
  }

  return { status, today, busy, error, summary, summaryFailed, update, openDataDirectory };
}
