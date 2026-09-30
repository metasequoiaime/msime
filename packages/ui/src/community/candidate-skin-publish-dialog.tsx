import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { boundedGraphemes } from "../core/text";
import { randomUuid } from "../core/random-id";
import type { SkinCatalog } from "../skin/external-skins";
import {
  candidateSkinMegabytes,
  candidateSkinMessage,
  communityNeedsSignIn,
} from "./community-helpers";
import * as style from "./community-style";
import { CommunitySkinPublicationFields } from "./community-skin-publication-fields";
import { CommunityErrorAlert } from "./community-error-alert";
import type {
  CandidateSkinCommunityClient,
  CandidateSkinPackPreview,
  CommunityCandidateSkin,
} from "./community-candidate-skins";

type LocalSkinOption = { id: string; name: string };

/** The server's package limit, stated next to the packed size so the author sees the headroom. */
const packageLimit = "2 MB";

const publishWarning =
  "仅上传 skin.toml 与 PNG/JPEG 图片（需包含预览图）；单个文件不超过 1 MB、合计不超过 2 MB、每边不超过 2048 像素。服务器会重新编码图片并去除元数据。发布后无法修改，更新请发布新作品。";

function licenseLines(license: CandidateSkinPackPreview["license"]): string[] {
  return [
    license.assets?.trim() ? `素材授权 ${license.assets.trim()}` : "",
    license.code?.trim() ? `代码授权 ${license.code.trim()}` : "",
    license.source?.trim() ? `来源 ${license.source.trim()}` : "",
  ].filter(Boolean);
}

/**
 * Publishes one of the user's installed candidate-window skin packages.
 *
 * The host packs the package itself from its own skin directory, so the webview only names the folder; `packPreview` runs the same checks the upload will, which lets a package the server would refuse say so before the form is even shown. The root is a `div`, not a `form`: from the 主题 page it renders inside the settings form, where a nested form is invalid and its submit would bubble into a preferences save.
 */
export function CandidateSkinPublishDialog({
  client,
  localSkins,
  initialSkinId,
  openSkinDirectory,
  onClose,
  onPublished,
  onLogin,
}: {
  client: CandidateSkinCommunityClient;
  /** The installed packages to choose from; absent, only `initialSkinId` is offered. */
  localSkins?: () => Promise<SkinCatalog>;
  initialSkinId?: string;
  openSkinDirectory?: () => Promise<void>;
  onClose: () => void;
  onPublished: (skin: CommunityCandidateSkin) => void | Promise<void>;
  /** Where to send someone who has to sign in before publishing; absent leaves the sentence alone. */
  onLogin?: () => void;
}) {
  const [options, setOptions] = useState<LocalSkinOption[]>(
    initialSkinId ? [{ id: initialSkinId, name: initialSkinId }] : [],
  );
  const [optionsLoading, setOptionsLoading] = useState(Boolean(localSkins));
  const [skinId, setSkinId] = useState(initialSkinId ?? "");
  const [pack, setPack] = useState<CandidateSkinPackPreview | null>(null);
  const [packError, setPackError] = useState("");
  const [packLoading, setPackLoading] = useState(false);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [agreed, setAgreed] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [signInRequired, setSignInRequired] = useState(false);
  const [openFailed, setOpenFailed] = useState(false);
  const [publicationId, setPublicationId] = useState(randomUuid);
  const clientGeneration = useRef(0);
  const packGeneration = useRef(0);

  useEffect(() => {
    const generation = ++clientGeneration.current;
    let active = true;
    if (localSkins) {
      setOptionsLoading(true);
      void localSkins()
        .then((catalog) => {
          if (!active) return;
          const loaded = catalog.packages.map((item) => ({ id: item.id, name: item.name }));
          setOptions(loaded);
          setSkinId((current) =>
            current && loaded.some((item) => item.id === current) ? current : (loaded[0]?.id ?? ""),
          );
        })
        .catch(() => {
          if (active) setError("读取本地皮肤失败，请重试。");
        })
        .finally(() => {
          if (active) setOptionsLoading(false);
        });
    }
    return () => {
      active = false;
      if (generation === clientGeneration.current) clientGeneration.current++;
    };
  }, [client, localSkins]);

  useEffect(() => {
    const generation = ++packGeneration.current;
    setPack(null);
    setPackError("");
    setAgreed(false);
    setPublicationId(randomUuid());
    if (!skinId) {
      setPackLoading(false);
      return;
    }
    setPackLoading(true);
    void client
      .packPreview(skinId)
      .then((value) => {
        if (generation !== packGeneration.current) return;
        setPack(value);
        setName(boundedGraphemes(value.suggestedName, 32));
      })
      .catch((packFailure) => {
        if (generation === packGeneration.current) setPackError(candidateSkinMessage(packFailure));
      })
      .finally(() => {
        if (generation === packGeneration.current) setPackLoading(false);
      });
    return () => {
      packGeneration.current++;
    };
  }, [client, skinId]);

  const normalizedName = name.trim();
  const normalizedDescription = description.trim();
  const nameValid =
    normalizedName.length > 0 &&
    boundedGraphemes(normalizedName, 32) === normalizedName &&
    [...normalizedName].length <= 32;
  const descriptionValid = [...normalizedDescription].length <= 280;
  const ready = Boolean(pack) && !packLoading && nameValid && descriptionValid && agreed;

  const submit = async () => {
    if (busy || !ready || !skinId) return;
    const generation = clientGeneration.current;
    setBusy(true);
    setError("");
    setSignInRequired(false);
    try {
      const published = await client.publish(
        skinId,
        publicationId,
        normalizedName,
        normalizedDescription,
      );
      if (generation !== clientGeneration.current) return;
      await onPublished(published);
    } catch (publishError) {
      if (generation !== clientGeneration.current) return;
      setError(candidateSkinMessage(publishError, true));
      setSignInRequired(communityNeedsSignIn(publishError));
      setBusy(false);
    }
  };

  const openFolder = async () => {
    if (!openSkinDirectory) return;
    setOpenFailed(false);
    try {
      await openSkinDirectory();
    } catch {
      setOpenFailed(true);
    }
  };

  // Enter in a text field would otherwise submit whichever form this dialog sits in.
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Enter" && event.target instanceof HTMLInputElement) {
      event.preventDefault();
      if (event.target.type !== "checkbox") void submit();
    }
  };

  return (
    <div className={style.backdrop}>
      <div
        className={style.dialog}
        role="dialog"
        aria-modal="true"
        aria-label="发布候选窗皮肤"
        onKeyDown={onKeyDown}
      >
        <div className={style.dialogHeading}>
          <h2 className={style.dialogTitle}>发布候选窗皮肤</h2>
          <button
            type="button"
            className={style.dialogClose}
            disabled={busy}
            onClick={onClose}
            aria-label="关闭发布窗口"
          >
            ×
          </button>
        </div>
        {error && (
          <CommunityErrorAlert message={error} signInRequired={signInRequired} onLogin={onLogin} />
        )}
        {optionsLoading && <p role="status">正在读取本地皮肤…</p>}
        {!optionsLoading && options.length === 0 && (
          <p className={style.notice}>
            还没有可发布的外部皮肤，请先把皮肤文件夹放进皮肤目录，再在「社区」的本地皮肤中刷新。
          </p>
        )}
        {options.length > 0 && (
          <label className={style.field}>
            发布皮肤
            <select
              className={style.fieldControl}
              aria-label="发布皮肤"
              value={skinId}
              disabled={busy}
              onChange={(event) => setSkinId(event.target.value)}
            >
              {options.map((item) => (
                <option key={item.id} value={item.id}>
                  {item.name === item.id ? item.id : `${item.name}（${item.id}）`}
                </option>
              ))}
            </select>
          </label>
        )}
        {packLoading && <p role="status">正在检查皮肤包…</p>}
        {packError && (
          <div className={style.confirmation} role="alert">
            <p>{packError}</p>
            {openSkinDirectory && (
              <div className={style.confirmationActions}>
                <button type="button" className="secondary" onClick={() => void openFolder()}>
                  打开目录
                </button>
              </div>
            )}
            {openFailed && <p>无法打开皮肤目录，请重试。</p>}
          </div>
        )}
        {pack && (
          <>
            <p className={style.metrics}>
              {pack.fileCount} 个文件 · {candidateSkinMegabytes(pack.size)} / {packageLimit}
            </p>
            <p className={style.metrics} aria-label="皮肤授权">
              {licenseLines(pack.license).join(" / ")}
            </p>
            <CommunitySkinPublicationFields
              name={name}
              description={description}
              agreed={agreed}
              busy={busy}
              agreementText="我拥有发布所用素材的权利，并同意其他用户按上述授权免费下载使用"
              onNameChange={(value) => {
                setPublicationId(randomUuid());
                setName(boundedGraphemes(value, 32));
              }}
              onDescriptionChange={(value) => {
                setPublicationId(randomUuid());
                setDescription(value);
              }}
              onAgreedChange={setAgreed}
            />
            <p className={style.warning}>{publishWarning}</p>
          </>
        )}
        <div className={style.dialogActions}>
          <button type="button" className="secondary" disabled={busy} onClick={onClose}>
            取消
          </button>
          {!packError && (
            <button
              type="button"
              className="primary"
              disabled={busy || !ready}
              onClick={() => void submit()}
            >
              {busy ? "正在发布…" : "公开发布"}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
