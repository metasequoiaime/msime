import { useState } from "react";
import { useMountedRef } from "../settings/use-mounted-ref";
import {
  resourcePackStatus,
  type ModelMirrorClient,
  type ResourcePacks,
} from "../settings/resource-packs";
import {
  formatModelBytes,
  localModelProgressPercent,
  validModelMirror,
} from "../voice/local-model-helpers";
import * as surface from "./panel-surface-style";

/** 手写模型下载中的进度与取消、下载失败的原因与重试（宿主能读写镜像时还有设置下载镜像的入口），或取消后仍缺模型时的下载入口；模型已在、也没有下载时不渲染。 */
export function HandwritingModelNotice({
  packs,
  mirror,
}: {
  packs: ResourcePacks;
  mirror?: ModelMirrorClient;
}) {
  const status = resourcePackStatus(packs, "handwriting");
  const progress = packs.progress.handwriting;
  const error = packs.errors.handwriting;
  if (progress) {
    return (
      <p className={surface.handwritingActions} aria-live="polite">
        <span>
          正在下载手写模型（约 {formatModelBytes(status?.size ?? progress.total)}）…{" "}
          {localModelProgressPercent(progress)}%
        </span>
        <button type="button" onClick={() => packs.cancel("handwriting")}>
          取消
        </button>
      </p>
    );
  }
  if (error) {
    return (
      <>
        <p className={surface.handwritingActions} aria-live="polite">
          <span>{error}</span>
          <button type="button" onClick={() => packs.install("handwriting")}>
            重试
          </button>
        </p>
        {mirror && (
          <HandwritingMirrorEditor mirror={mirror} onSaved={() => packs.install("handwriting")} />
        )}
      </>
    );
  }
  if (status?.state === "missing") {
    return (
      <p className={surface.handwritingActions} aria-live="polite">
        <span>手写识别需要先下载手写模型（约 {formatModelBytes(status.size)}）</span>
        <button type="button" onClick={() => packs.install("handwriting")}>
          下载
        </button>
      </p>
    );
  }
  return null;
}

/** 手写模型下载失败后的镜像设置：先是一个「设置下载镜像」按钮，点开后读出已保存的镜像、就地编辑，「保存并重试」写入偏好后重新下载。 */
function HandwritingMirrorEditor({
  mirror,
  onSaved,
}: {
  mirror: ModelMirrorClient;
  onSaved: () => void;
}) {
  const [value, setValue] = useState<string>();
  const [message, setMessage] = useState("");
  const [saving, setSaving] = useState(false);
  const mounted = useMountedRef();
  async function open() {
    try {
      const saved = await mirror.load();
      if (mounted.current) setValue(saved);
    } catch {
      if (mounted.current) setMessage("无法读取下载镜像，请重试。");
    }
  }
  async function save() {
    if (value === undefined || saving) return;
    const next = value.trim();
    if (!validModelMirror(next)) {
      setMessage("下载镜像地址无效，必须以 https:// 开头。");
      return;
    }
    setSaving(true);
    setMessage("");
    try {
      await mirror.save(next);
      if (!mounted.current) return;
      onSaved();
    } catch {
      if (mounted.current) setMessage("无法保存下载镜像，请重试。");
    } finally {
      if (mounted.current) setSaving(false);
    }
  }
  if (value === undefined) {
    return (
      <p className={surface.handwritingActions}>
        {message && <span>{message}</span>}
        <button type="button" onClick={() => void open()}>
          设置下载镜像
        </button>
      </p>
    );
  }
  return (
    <p className={surface.handwritingActions}>
      <input
        aria-label="模型下载镜像"
        placeholder="https://mirror.example.com"
        maxLength={2048}
        value={value}
        aria-invalid={!validModelMirror(value.trim())}
        disabled={saving}
        onChange={(event) => setValue(event.target.value)}
      />
      <button type="button" disabled={saving} onClick={() => void save()}>
        保存并重试
      </button>
      {message && <span>{message}</span>}
    </p>
  );
}
