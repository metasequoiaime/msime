import { useEffect, useRef, useState } from "react";
import { boundedGraphemes, utf8ByteLength } from "../core/text";
import { errorCode } from "../core/error-code";
import { useAsyncActionRunner } from "../core/use-async-action";
import { useAsyncGeneration } from "../settings/use-async-generation";
import type { ExternalSkin, SkinCatalog } from "../skin/external-skins";
import type { SkinImageReader } from "../skin/skin-image";
import { renderSkinPreview } from "../skin/skin-preview-render";
import { candidateSkinMessage, communityNeedsSignIn } from "./community-helpers";
import { communityPublishFields } from "./community-publish-validation";
import { useCommunityPublicationDraft } from "./use-community-publication-draft";
import type {
  CandidateSkinCategory,
  CandidateSkinCommunityClient,
  CandidateSkinPackPreview,
  CandidateSkinVisibility,
  CommunityCandidateSkin,
} from "./community-candidate-skin-model";

export interface CandidateSkinPublicationOptions {
  client: Pick<
    CandidateSkinCommunityClient,
    "packPreview" | "addPreview" | "addLicense" | "publish"
  >;
  /** 本机可选皮肤；未提供时仅使用初始皮肤。 */
  localSkins?: () => Promise<SkinCatalog>;
  initialSkinId?: string;
  openSkinDirectory?: () => Promise<void>;
  /** 读取包内图片，为缺少预览的皮肤生成预览图。 */
  readImage?: SkinImageReader;
  onPublished: (skin: CommunityCandidateSkin) => void | Promise<void>;
}

type LocalSkinOption = { id: string; name: string };

/** 公开包缺少素材授权时提供的选项；自定义值按宿主的 UTF-8 字节上限校验。 */
export const candidateSkinAssetLicenses = [
  { value: "CC-BY-4.0", label: "CC BY 4.0（推荐：可自由使用，需署名作者）" },
  { value: "CC0-1.0", label: "CC0 1.0（放弃权利，任何人可随意使用）" },
] as const;
const assetLicenseLimit = 120;

/** 候选皮肤的预检、修补和发布状态；调用方持有表单事件与成功后的导航。 */
export function useCandidateSkinPublication({
  client,
  localSkins,
  initialSkinId,
  openSkinDirectory,
  readImage,
  onPublished,
}: CandidateSkinPublicationOptions) {
  const [options, setOptions] = useState<LocalSkinOption[]>(
    initialSkinId ? [{ id: initialSkinId, name: initialSkinId }] : [],
  );
  const [packages, setPackages] = useState<ExternalSkin[]>([]);
  const [optionsLoading, setOptionsLoading] = useState(Boolean(localSkins));
  const [skinId, setSkinId] = useState(initialSkinId ?? "");
  const [pack, setPack] = useState<CandidateSkinPackPreview | null>(null);
  const [packError, setPackError] = useState("");
  const [packCode, setPackCode] = useState<string | undefined>();
  // 保存生成的预览后递增，重新检查皮肤包。
  const [packRevision, setPackRevision] = useState(0);
  const [drawing, setDrawing] = useState(false);
  const [drawFailed, setDrawFailed] = useState(false);
  const [licenseChoice, setLicenseChoice] = useState<string>(candidateSkinAssetLicenses[0].value);
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
  const [error, setError] = useState("");
  const [signInRequired, setSignInRequired] = useState(false);
  const [openFailed, setOpenFailed] = useState(false);
  const {
    busy,
    generation: clientGeneration,
    running: actionRunning,
    run: runAsyncAction,
  } = useAsyncActionRunner(setError, undefined, client, localSkins);
  const packGeneration = useAsyncGeneration(client, skinId, visibility, packRevision);
  const drawRunning = useRef(false);
  const drawOwner = useAsyncGeneration();
  const licenseRunning = useRef(false);
  const licenseOwner = useAsyncGeneration();
  // 记录名称来自哪个包；切换可见性重新预检时保留用户手工输入的名称。
  const namedSkin = useRef("");

  useEffect(() => {
    const generation = clientGeneration.current;
    drawOwner.current++;
    drawRunning.current = false;
    licenseOwner.current++;
    licenseRunning.current = false;
    setDrawing(false);
    setWritingLicense(false);
    if (localSkins) {
      setOptionsLoading(true);
      void localSkins()
        .then((catalog) => {
          if (generation !== clientGeneration.current) return;
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
          if (generation === clientGeneration.current) setError("读取本地皮肤失败，请重试。");
        })
        .finally(() => {
          if (generation === clientGeneration.current) setOptionsLoading(false);
        });
    }
  }, [client, clientGeneration, localSkins]);

  useEffect(() => {
    const generation = packGeneration.current;
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
  }, [client, skinId, visibility, packGeneration, packRevision]);

  // 缺少独立装饰图的皮肤会把预览当作装饰；自动生成会改变外观，宿主也拒绝这种补写。
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

  // 只有公开包需要素材授权；宿主把所选授权写入 skin.toml，作者无需手工编辑清单。
  const licenseless = packCode === "candidate_skin_license_required" && skinId !== "";
  const licenseValue = licenseChoice === "other" ? customLicense.trim() : licenseChoice;
  const licenseValid = licenseValue.length > 0 && utf8ByteLength(licenseValue) <= assetLicenseLimit;
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
    setSignInRequired(false);
    await runAsyncAction(
      async (isCurrent) => {
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

  const changeCategory = (next: CandidateSkinCategory) => {
    // 分类属于发布内容，改变分类后需要新的发布 ID。
    resetPublication();
    setCategory(next);
  };

  return {
    options,
    optionsLoading,
    skinId,
    pack,
    packError,
    packLoading,
    previewless,
    drawing,
    drawFailed,
    licenseless,
    licenseChoice,
    customLicense,
    licenseValid,
    writingLicense,
    licenseFailed,
    name,
    description,
    agreed,
    visibility,
    category,
    error,
    signInRequired,
    openFailed,
    busy,
    ready,
    setSkinId,
    setVisibility,
    setLicenseChoice,
    setCustomLicense,
    onNameChange,
    onDescriptionChange,
    onAgreedChange,
    changeCategory,
    drawPreview,
    writeLicense,
    submit,
    openFolder,
  };
}
