import { useEffect, useRef, useState } from "react";
import { boundedGraphemes } from "../core/text";
import {
  communityPublishFields,
  handleCommunityPublishKeyDown,
} from "./community-publish-validation";
import { errorCode } from "../core/error-code";
import type { ExternalSkin, SkinCatalog } from "../skin/external-skins";
import type { SkinImageReader } from "../skin/skin-image";
import { renderSkinPreview } from "../skin/skin-preview-render";
import {
  candidateSkinMegabytes,
  candidateSkinMessage,
  communityLicenseLine,
  runCommunityPublishAction,
} from "./community-helpers";
import * as style from "./community-style";
import { CommunitySkinPublicationFields } from "./community-skin-publication-fields";
import { CommunityPublicationWarning } from "./community-publication-warning";
import { CommunityNotice } from "./community-notice";
import { CommunityDialogActions, CommunityDialogFrame } from "./community-dialog";
import { CommunitySkinCategorySelect } from "./community-skin-category";
import { CommunitySelectField } from "./community-select-field";
import { ActionButton } from "../core/action-button";
import { useCommunityPublicationDraft } from "./use-community-publication-draft";
import {
  type CandidateSkinCategory,
  type CandidateSkinCommunityClient,
  type CandidateSkinPackPreview,
  type CandidateSkinVisibility,
  type CommunityCandidateSkin,
} from "./community-candidate-skins";

type LocalSkinOption = { id: string; name: string };

/** The server's package limit, stated next to the packed size so the author sees the headroom. */
const packageLimit = "2 MB";

const publishWarning =
  "仅上传 skin.toml 与 PNG/JPEG 图片（需包含预览图）；单个文件不超过 1 MB、合计不超过 2 MB、每边不超过 2048 像素。服务器会重新编码图片并去除元数据。登录期间修改本地皮肤，会自动同步到这款作品。";

/** Asset licenses offered when a public package names none; the host bounds the manifest value to this many UTF-8 bytes. */
const assetLicenses = [
  { value: "CC-BY-4.0", label: "CC BY 4.0（推荐：可自由使用，需署名作者）" },
  { value: "CC0-1.0", label: "CC0 1.0（放弃权利，任何人可随意使用）" },
] as const;
const assetLicenseLimit = 120;

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
  const [licenseChoice, setLicenseChoice] = useState<string>(assetLicenses[0].value);
  const [customLicense, setCustomLicense] = useState("");
  const [writingLicense, setWritingLicense] = useState(false);
  const [licenseFailed, setLicenseFailed] = useState(false);
  const [packLoading, setPackLoading] = useState(false);
  const {
    name,
    description,
    agreed,
    publicationId,
    setName,
    setAgreed,
    onNameChange,
    onDescriptionChange,
    onAgreedChange,
    resetPublication,
  } = useCommunityPublicationDraft();
  const [visibility, setVisibility] = useState<CandidateSkinVisibility>("public");
  const [category, setCategory] = useState<CandidateSkinCategory>("other");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [signInRequired, setSignInRequired] = useState(false);
  const [openFailed, setOpenFailed] = useState(false);
  const clientGeneration = useRef(0);
  const packGeneration = useRef(0);
  const actionRunning = useRef(false);
  const drawRunning = useRef(false);
  const drawOwner = useRef(0);
  const licenseRunning = useRef(false);
  const licenseOwner = useRef(0);
  // The package whose name the form was filled from, so switching visibility re-checks the package without discarding a name the user typed.
  const namedSkin = useRef("");

  useEffect(() => {
    const generation = ++clientGeneration.current;
    let active = true;
    actionRunning.current = false;
    drawOwner.current++;
    drawRunning.current = false;
    licenseOwner.current++;
    licenseRunning.current = false;
    setBusy(false);
    setDrawing(false);
    setWritingLicense(false);
    if (localSkins) {
      setOptionsLoading(true);
      void localSkins()
        .then((catalog) => {
          if (!active) return;
          const loaded = catalog.packages.map((item) => ({
            id: item.id,
            name: item.name,
          }));
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
    setLicenseFailed(false);
    setAgreed(false);
    resetPublication();
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
    if (!previewless || !readImage || drawing || drawRunning.current) return;
    const owner = ++drawOwner.current;
    drawRunning.current = true;
    const clientGenerationAtStart = clientGeneration.current;
    const packGenerationAtStart = packGeneration.current;
    setDrawing(true);
    setDrawFailed(false);
    try {
      const bytes = await renderSkinPreview(previewless, readImage);
      const catalog = await client.addPreview(previewless.id, bytes);
      if (
        clientGenerationAtStart !== clientGeneration.current ||
        packGenerationAtStart !== packGeneration.current
      )
        return;
      setPackages(catalog.packages);
      setPackRevision((revision) => revision + 1);
    } catch {
      if (
        packGenerationAtStart === packGeneration.current &&
        clientGenerationAtStart === clientGeneration.current
      )
        setDrawFailed(true);
    } finally {
      if (drawOwner.current === owner) drawRunning.current = false;
      if (
        packGenerationAtStart === packGeneration.current &&
        clientGenerationAtStart === clientGeneration.current
      )
        setDrawing(false);
      else if (drawOwner.current === owner) setDrawing(false);
    }
  };

  // Only a public package needs an asset license; the host writes the chosen one into skin.toml, so the author never edits the manifest by hand.
  const licenseless = packCode === "candidate_skin_license_required" && skinId !== "";
  const licenseValue = licenseChoice === "other" ? customLicense.trim() : licenseChoice;
  const licenseValid =
    licenseValue.length > 0 && new TextEncoder().encode(licenseValue).length <= assetLicenseLimit;
  const writeLicense = async () => {
    if (!licenseless || !licenseValid || writingLicense || licenseRunning.current) return;
    const owner = ++licenseOwner.current;
    licenseRunning.current = true;
    const clientGenerationAtStart = clientGeneration.current;
    const packGenerationAtStart = packGeneration.current;
    setWritingLicense(true);
    setLicenseFailed(false);
    try {
      const catalog = await client.addLicense(skinId, licenseValue);
      if (
        clientGenerationAtStart !== clientGeneration.current ||
        packGenerationAtStart !== packGeneration.current
      )
        return;
      setPackages(catalog.packages);
      setPackRevision((revision) => revision + 1);
    } catch {
      if (
        packGenerationAtStart === packGeneration.current &&
        clientGenerationAtStart === clientGeneration.current
      )
        setLicenseFailed(true);
    } finally {
      if (licenseOwner.current === owner) licenseRunning.current = false;
      if (
        packGenerationAtStart === packGeneration.current &&
        clientGenerationAtStart === clientGeneration.current
      )
        setWritingLicense(false);
      else if (licenseOwner.current === owner) setWritingLicense(false);
    }
  };

  const { normalizedName, normalizedDescription, nameValid, descriptionValid } =
    communityPublishFields(name, description);
  const ready = Boolean(pack) && !packLoading && nameValid && descriptionValid && agreed;

  const submit = async () => {
    if (busy || actionRunning.current || !ready || !skinId) return;
    const generation = clientGeneration.current;
    await runCommunityPublishAction({
      busy,
      generation,
      clientGeneration,
      actionRunning,
      setBusy,
      setError,
      setSignInRequired,
      formatError: (publishError) => candidateSkinMessage(publishError, true),
      operation: async (isCurrent) => {
        const published = await client.publish(
          skinId,
          publicationId,
          normalizedName,
          normalizedDescription,
          visibility,
          category,
        );
        if (!isCurrent()) return;
        await onPublished(published);
      },
    });
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

  return (
    <CommunityDialogFrame
      title="发布候选窗口皮肤"
      titleClassName={style.dialogTitle}
      ariaLabel="发布候选窗口皮肤"
      busy={busy}
      onClose={onClose}
      error={error}
      signInRequired={signInRequired}
      onLogin={onLogin}
      onKeyDown={(event) => handleCommunityPublishKeyDown(event, () => void submit())}
    >
      {optionsLoading && <p role="status">正在读取本地皮肤…</p>}
      {!optionsLoading && options.length === 0 && (
        <CommunityNotice>
          还没有可发布的外部皮肤，请先把皮肤文件夹放进皮肤目录，再在「主题」的外部皮肤中刷新。
        </CommunityNotice>
      )}
      {options.length > 0 && (
        <CommunitySelectField
          label="发布皮肤"
          ariaLabel="发布皮肤"
          value={skinId}
          disabled={busy}
          onChange={setSkinId}
        >
          {options.map((item) => (
            <option key={item.id} value={item.id}>
              {item.name === item.id ? item.id : `${item.name}（${item.id}）`}
            </option>
          ))}
        </CommunitySelectField>
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
              <ActionButton
                action={() => void openFolder()}
                className="secondary"
                disabled={drawing}
                label="打开目录"
              />
            )}
            <ActionButton
              action={() => void drawPreview()}
              className="primary"
              disabled={drawing}
              label={drawing ? "正在生成…" : "生成预览图"}
            />
          </div>
          {openFailed && <p>无法打开皮肤目录，请重试。</p>}
        </div>
      )}
      {packError && licenseless && (
        <fieldset className={style.field} disabled={writingLicense}>
          <legend>素材授权</legend>
          <p className={style.metrics}>
            公开发布需要注明别人可以怎样使用皮肤里的图片，选择后会写入 skin.toml。
          </p>
          {assetLicenses.map((item) => (
            <label key={item.value}>
              <input
                type="radio"
                name="candidate-skin-asset-license"
                checked={licenseChoice === item.value}
                onChange={() => setLicenseChoice(item.value)}
              />{" "}
              {item.label}
            </label>
          ))}
          <label>
            <input
              type="radio"
              name="candidate-skin-asset-license"
              checked={licenseChoice === "other"}
              onChange={() => setLicenseChoice("other")}
            />{" "}
            其他
          </label>
          {licenseChoice === "other" && (
            <input
              className={style.fieldControl}
              aria-label="其他素材授权"
              placeholder="例如：仅限个人使用，不得转售"
              value={customLicense}
              onChange={(event) => setCustomLicense(event.target.value)}
            />
          )}
          {licenseChoice === "other" && customLicense.trim() !== "" && !licenseValid && (
            <p className={style.metrics} role="alert">
              授权说明太长，请控制在 40 个汉字以内。
            </p>
          )}
          {licenseFailed && (
            <p className={style.metrics} role="alert">
              写入授权失败，请重试，或在 skin.toml 的 [license] 中自己填写 assets。
            </p>
          )}
        </fieldset>
      )}
      {packError && !(previewless && readImage) && !licenseless && (
        <div className={style.confirmation} role="alert">
          <p>{packError}</p>
          {openSkinDirectory && (
            <div className={style.confirmationActions}>
              <ActionButton
                action={() => void openFolder()}
                className="secondary"
                label="打开目录"
              />
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
            {communityLicenseLine(pack.license)}
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
            onNameChange={onNameChange}
            onDescriptionChange={onDescriptionChange}
            onAgreedChange={onAgreedChange}
          />
          <CommunitySkinCategorySelect
            ariaLabel="发布分类"
            value={category}
            disabled={busy}
            onChange={(next) => {
              // 分类也是这次发布的内容，换了分类就是另一次发布，不能沿用上一次的发布 id。
              resetPublication();
              setCategory(next);
            }}
          />
          <CommunityPublicationWarning>{publishWarning}</CommunityPublicationWarning>
        </>
      )}
      <CommunityDialogActions busy={busy} onClose={onClose}>
        {packError && licenseless && (
          <ActionButton
            action={() => void writeLicense()}
            className="primary"
            disabled={writingLicense || !licenseValid}
            label={writingLicense ? "正在写入…" : "使用此授权并继续"}
          />
        )}
        {!packError && (
          <ActionButton
            action={() => void submit()}
            className="primary"
            disabled={busy || !ready}
            label={busy ? "正在发布…" : visibility === "public" ? "公开发布" : "保存到我的皮肤库"}
          />
        )}
      </CommunityDialogActions>
    </CommunityDialogFrame>
  );
}
