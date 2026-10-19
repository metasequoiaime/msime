import type { CloudCandidate, CloudDictionaryKind, CloudFixedPosition } from "./panel-client";
import { CloudDictionaryItem } from "./cloud-dictionary-item";
import { candidateMutationCode } from "./cloud-response";
import * as cloud from "./cloud-panel-style";

export interface CloudCandidateResultsProps {
  kind: CloudDictionaryKind;
  queryText: string;
  revision: number;
  candidates: readonly CloudCandidate[];
  busy: boolean;
  position: number;
  onRank: (candidate: CloudCandidate) => void;
  onSetFixed: (candidate: CloudCandidate, position: number) => void;
  onRemove: (candidate: CloudCandidate) => void;
}

export function CloudCandidateResults({
  kind,
  queryText,
  revision,
  candidates,
  busy,
  position,
  onRank,
  onSetFixed,
  onRemove,
}: CloudCandidateResultsProps) {
  return (
    <>
      <div className={cloud.dictionaryList} aria-label="云端候选结果">
        <p>
          查询编码：{queryText} · 云端版本 {revision}
        </p>
        {candidates.length ? (
          candidates.map((candidate, index) => (
            <CloudDictionaryItem
              key={`${candidateMutationCode(candidate)}:${candidate.word}`}
              ariaLabel={`调频候选 ${candidate.word}`}
              busy={busy}
              disabled={kind === "quick"}
              onClick={() => void onRank(candidate)}
              actions={
                kind !== "quick" && (
                  <>
                    <button
                      type="button"
                      className="secondary"
                      onClick={() => void onRank(candidate)}
                      disabled={busy}
                    >
                      调频
                    </button>
                    <button
                      type="button"
                      className="secondary"
                      onClick={() => void onSetFixed(candidate, position)}
                      disabled={busy}
                    >
                      固定
                    </button>
                    <button
                      type="button"
                      className="secondary"
                      onClick={() => void onRemove(candidate)}
                      disabled={
                        busy || (kind !== "english" && Array.from(candidate.word).length <= 1)
                      }
                    >
                      删除
                    </button>
                  </>
                )
              }
            >
              <strong>
                {index + 1}. <span className="wrap-anywhere">{candidate.word}</span>
              </strong>
              <small>
                {candidate.code} · 权重 {candidate.weight}
              </small>
            </CloudDictionaryItem>
          ))
        ) : (
          <p className={cloud.dictionaryEmpty}>没有匹配的候选</p>
        )}
      </div>
      {candidates.length > 0 && (
        <p className={cloud.dictionaryMobileHint}>
          点按候选可调频；窄屏下的固定和删除操作会分组显示。
        </p>
      )}
    </>
  );
}

export interface CloudFixedPositionsProps {
  items: readonly CloudFixedPosition[];
  busy: boolean;
  onUnfix: (item: CloudFixedPosition) => void;
}

export function CloudFixedPositions({ items, busy, onUnfix }: CloudFixedPositionsProps) {
  if (!items.length) return null;
  return (
    <div className={cloud.dictionaryList} aria-label="固定位置">
      <p>此查询的固定位置</p>
      {items.map((item) => (
        <article className={cloud.dictionaryItem} key={`${item.context}:${item.code}:${item.word}`}>
          <div className={`${cloud.dictionaryItemMain} ${cloud.dictionaryItemStatic}`}>
            <strong>
              第 {item.position} 位 · {item.word}
            </strong>
            <small>{item.code}</small>
          </div>
          <div className={cloud.dictionaryItemActions}>
            <button
              type="button"
              className="secondary"
              onClick={() => void onUnfix(item)}
              disabled={busy}
            >
              取消固定
            </button>
          </div>
        </article>
      ))}
    </div>
  );
}
