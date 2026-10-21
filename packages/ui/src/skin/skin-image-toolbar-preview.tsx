import { useEffect, useState } from "react";
import type { FloatingToolbarPreferences } from "../settings/floating-toolbar-preferences";
import { useSkinImage, type SkinImageReader } from "./skin-image";
import {
  toolbarImageLayout,
  toolbarPreviewRequests,
  type SkinToolbarImage,
  type ToolbarImageSlot,
} from "./toolbar-image-layout";

function ToolbarImage({
  readImage,
  id,
  path,
  revision,
}: {
  readImage?: SkinImageReader;
  id: string;
  path: string;
  revision: number;
}) {
  const image = useSkinImage(readImage, id, path, revision);
  const [failed, setFailed] = useState<string>();
  useEffect(() => setFailed(undefined), [readImage, id, path, revision]);
  return image?.url && failed !== image.url ? (
    <img
      alt=""
      src={image.url}
      draggable={false}
      onError={() => setFailed(image.url)}
      style={{
        position: "absolute",
        inset: 0,
        width: "100%",
        height: "100%",
        objectFit: "fill",
        pointerEvents: "none",
      }}
    />
  ) : null;
}

function ToolbarButton({
  slot,
  id,
  readImage,
  revision,
}: {
  slot: ToolbarImageSlot;
  id: string;
  readImage?: SkinImageReader;
  revision: number;
}) {
  const face = {
    status: "中",
    fullwidth: "半",
    punctuation: "。，",
    character_set: "简",
    emoji: "☺",
    screen_keyboard: "⌨",
    settings: "⚙",
  };
  const image = useSkinImage(readImage, id, slot.image, revision);
  const [failed, setFailed] = useState<string>();
  useEffect(() => setFailed(undefined), [readImage, id, slot.image, revision]);
  const drawn = image?.url && failed !== image.url;
  return (
    <span
      data-toolbar-image-button={slot.key}
      style={{
        position: "absolute",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        left: slot.xDip,
        top: slot.yDip,
        width: slot.widthDip,
        height: slot.heightDip,
        fontSize: slot.iconSizeDip,
        lineHeight: 1,
      }}
    >
      {slot.background && (
        <ToolbarImage readImage={readImage} id={id} path={slot.background} revision={revision} />
      )}
      {drawn ? (
        <img
          alt=""
          src={image.url}
          draggable={false}
          onError={() => setFailed(image.url)}
          style={{
            position: "absolute",
            inset: 0,
            width: "100%",
            height: "100%",
            objectFit: "fill",
          }}
        />
      ) : (
        <span style={{ position: "relative" }}>{face[slot.key]}</span>
      )}
    </span>
  );
}

/** 图片预览不套用条形工具栏的底色、阴影、把手和分隔线。 */
export function SkinImageToolbarPreview({
  skin,
  id,
  readImage,
  revision = 0,
  preferences,
  theme = "dark",
}: {
  skin: SkinToolbarImage;
  id: string;
  readImage?: SkinImageReader;
  revision?: number;
  preferences?: FloatingToolbarPreferences;
  theme?: "dark" | "light";
}) {
  const layout = toolbarImageLayout(skin, toolbarPreviewRequests(preferences));
  const scale = (preferences?.scale_percent ?? 100) / 100;
  return (
    <div
      aria-hidden="true"
      data-toolbar-image-preview=""
      style={{
        width: layout.widthDip * scale,
        height: layout.heightDip * scale,
        flexShrink: 0,
        pointerEvents: "none",
        color: `var(--msime-toolbar-icon, ${theme === "light" ? "#1a1a1a" : "#fff"})`,
      }}
    >
      <div
        style={{
          position: "relative",
          width: layout.widthDip,
          height: layout.heightDip,
          transform: `scale(${scale})`,
          transformOrigin: "top left",
        }}
      >
        <div
          style={{ position: "absolute", width: skin.imageWidthDip, height: skin.imageHeightDip }}
        >
          <ToolbarImage readImage={readImage} id={id} path={skin.image} revision={revision} />
        </div>
        {layout.buttons.map((slot) => (
          <ToolbarButton
            key={slot.key}
            slot={slot}
            id={id}
            readImage={readImage}
            revision={revision}
          />
        ))}
      </div>
    </div>
  );
}
