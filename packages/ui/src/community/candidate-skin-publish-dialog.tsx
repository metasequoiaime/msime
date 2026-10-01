import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { boundedGraphemes } from "../core/text";
import { randomUuid } from "../core/random-id";
import { errorCode } from "../core/error-code";
import { runAsyncAction } from "../core/async-action";
import type { ExternalSkin, SkinCatalog } from "../skin/external-skins";
import type { SkinImageReader } from "../skin/skin-image";
import { renderSkinPreview } from "../skin/skin-preview-render";
import {
  candidateSkinMegabytes,
  candidateSkinMessage,
  communityNeedsSignIn,
} from "./community-helpers";
import * as style from "./community-style";
import { CommunitySkinPublicationFields } from "./community-skin-publication-fields";
import { CommunityErrorAlert } from "./community-error-alert";
import { CommunityDialogHeader } from "./community-dialog";
import type {
  CandidateSkinCommunityClient,
  CandidateSkinPackPreview,
  CandidateSkinVisibility,
  CommunityCandidateSkin,
} from "./community-candidate-skins";

type LocalSkinOption = { id: string; name: string };

/** The server's package limit, stated next to the packed size so the author sees the headroom. */
const packageLimit = "2 MB";

const publishWarning =
  "仅上传 skin.toml 与 PNG/JPEG 图片（需包含预览图）；单个文件不超过 1 MB、合计不超过 2 MB、每边不超过 2048 像素。服务器会重新编码图片并去除元数据。登录期间修改本地皮肤，会自动同步到这款作品。";

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
  readImage,
  onClose,
  onPublished,
  onLogin,
}: {
  client: CandidateSkinCommunityClient;
  /** The installed packages to choose from; absent, only `initialSkinId` is offered. */
  localSkins?: () => Promise<SkinCatalog>;
  initialSkinId?: string;
  openSkinDirectory?: () => Promise<void>;
  /** Reads a package's images, so a package without a preview can have one drawn from its own; absent, it is only told to add one. */
  readImage?: SkinImageReader;
  onClose: () => void;
  onPublished: (skin: CommunityCandidateSkin) => void | Promise<void>;
  /** Where to send someone who has to sign in before publishing; absent leaves the sentence alone. */
  onLogin?: () => void;
}) {
  const [options, setOptions] = useState<LocalSkinOption[]>(
    initialSkinId ? [{ id: initialSkinId, name: initialSkinId }] : [],
  );
  const [packages, setPackages] = useState<ExternalSkin[]>([]);
  const [optionsLoading, setOptionsLoading] = useState(Boolean(localSkins));
  const [skinId, setSkinId] = useState(initialSkinId ?? "");
  const [pack, setPack] = useState<CandidateSkinPackPreview | null>(null);
  const [packError, setPackError] = useState("");
  const [packCode, setPackCode] = useState<string | undefined>();
  // Bumped once a drawn preview is saved, so the package is checked again.
  const [packRevision, setPackRevision] = useState(0);
  const [drawing, setDrawing] = useState(false);
  const [drawFailed, setDrawFailed] = useState(false);
  const [packLoading, setPackLoading] = useState(false);
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [agreed, setAgreed] = useState(false);
  const [visibility, setVisibility] = useState<CandidateSkinVisibility>("public");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [signInRequired, setSignInRequired] = useState(false);
  const [openFailed, setOpenFailed] = useState(false);
  const [publicationId, setPublicationId] = useState(randomUuid);
  const clientGeneration = useRef(0);
  const packGeneration = useRef(0);
  // The package whose name the form was filled from, so switching visibility re-checks the package without discarding a name the user typed.
  const namedSkin = useRef("");

  useEffect(() => {
    const generation = ++clientGeneration.current;
    let active = true;
    if (localSkins) {
      setOptionsLoading(true);
      void localSkins()
        .then((catalog) => {
          if (!active) return;
          const loaded = catalog.packages.map((item) => ({ id: item.id, name: item.name }));
          setPackages(catalog.packages);
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
    setPackCode(undefined);
    setDrawFailed(false);
    setAgreed(false);
    setPublicationId(randomUuid());
    if (!skinId) {
      setPackLoading(false);
      return;
    }
    setPackLoading(true);
    void client
      .packPreview(skinId, visibility)
      .then((value) => {
        if (generation !== packGeneration.current) return;
        setPack(value);
        if (namedSkin.current !== skinId) {
          namedSkin.current = skinId;
          setName(boundedGraphemes(value.suggestedName, 32));
        }
      })
      .catch((packFailure) => {
        if (generation !== packGeneration.current) return;
        setPackError(candidateSkinMessage(packFailure));
        setPackCode(errorCode(packFailure));
      })
      .finally(() => {
        if (generation === packGeneration.current) setPackLoading(false);
      });
    return () => {
      packGeneration.current++;
    };
  }, [client, skinId, visibility, packRevision]);

  // A decorated package without an image of its own draws its preview as the decoration, so a drawn preview would change its look; the host refuses it too.
  const previewless =
    packCode === "candidate_skin_preview_required"
      ? packages.find(
          (item) =>
            item.id === skinId &&
            !(item.decorationTopDip > 0 && item.decorationWidthDip > 0 && !item.decorationImage),
        )
      : undefined;
  const drawPreview = async () => {
    if (!previewless || !readImage || drawing) return;
    const generation = packGeneration.current;
    setDrawing(true);
    setDrawFailed(false);
    try {
      const bytes = await renderSkinPreview(previewless, readImage);
      const catalog = await client.addPreview(previewless.id, bytes);
      setPackages(catalog.packages);
      setPackRevision((revision) => revision + 1);
    } catch {
      if (generation === packGeneration.current) setDrawFailed(true);
    } finally {
      setDrawing(false);
    }
  };

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
    setSignInRequired(false);
    await runAsyncAction(
      {
        busy,
        isCurrent: () => generation === clientGeneration.current,
        setBusy,
        setError,
      },
      async (isCurrent) => {
        const published = await client.publish(
          skinId,
          publicationId,
          normalizedName,
          normalizedDescription,
          visibility,
        );
        if (!isCurrent()) return;
        await onPublished(published);
      },
      {
        formatError: (publishError) => candidateSkinMessage(publishError, true),
        onError: (publishError) => setSignInRequired(communityNeedsSignIn(publishError)),
      },
    );
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
        <CommunityDialogHeader
          title="发布候选窗皮肤"
          titleClassName={style.dialogTitle}
          busy={busy}
          onClose={onClose}
        />
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
        <fieldset className={style.field} disabled={busy}>
          <legend>谁可以看到</legend>
          <label>
            <input
              type="radio"
              name="candidate-skin-visibility"
              checked={visibility === "public"}
              onChange={() => setVisibility("public")}
            />{" "}
            公开（所有人可下载）
          </label>
          <label>
            <input
              type="radio"
              name="candidate-skin-visibility"
              checked={visibility === "private"}
              onChange={() => setVisibility("private")}
            />{" "}
            仅自己可见
          </label>
        </fieldset>
        {packLoading && <p role="status">正在检查皮肤包…</p>}
        {packError && previewless && readImage && (
          <div className={style.confirmation} role="alert">
            <p>
              这款皮肤还没有预览图，社区要用它展示皮肤。可以按皮肤自己的配色和图片生成一张，保存到皮肤文件夹后继续发布。
            </p>
            {drawFailed && (
              <p>生成预览图失败，请重试，或自己在 skin.toml 中用 preview 指定一张图片。</p>
            )}
            <div className={style.confirmationActions}>
              {openSkinDirectory && (
                <button
                  type="button"
                  className="secondary"
                  disabled={drawing}
                  onClick={() => void openFolder()}
                >
                  打开目录
                </button>
              )}
              <button
                type="button"
                className="primary"
                disabled={drawing}
                onClick={() => void drawPreview()}
              >
                {drawing ? "正在生成…" : "生成预览图"}
              </button>
            </div>
            {openFailed && <p>无法打开皮肤目录，请重试。</p>}
          </div>
        )}
        {packError && !(previewless && readImage) && (
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
              agreementText={
                visibility === "public"
                  ? "我拥有发布所用素材的权利，并同意其他用户按上述授权免费下载使用"
                  : "我拥有上传所用素材的权利"
              }
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
              {busy ? "正在发布…" : visibility === "public" ? "公开发布" : "保存到我的皮肤库"}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
