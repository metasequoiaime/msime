import { useEffect, useState } from "react";
import { installToolbarCss } from "./skin-toolbar-css";
import { prepareToolbarImages } from "./toolbar-images";
import { skinImageUrl, type SkinImageReader } from "./skin-image";
import { skinFontBytes, type SkinFontReader } from "./skin-font";
import { prepareToolbarFonts } from "./toolbar-fonts";
import { prepareToolbarImports } from "./toolbar-imports";
import { useAsyncGeneration } from "../settings/use-async-generation";

export type ToolbarCssReader = (id: string, relative?: string) => Promise<string | null>;
export function useToolbarCss(
  read: ToolbarCssReader | undefined,
  id: string,
  filename: string | null,
  revision: number,
  scope: string,
  readImage?: SkinImageReader,
  readFont?: SkinFontReader,
) {
  const [state, setState] = useState<"idle" | "loading" | "ready" | "partial" | "failed">("idle");
  const generation = useAsyncGeneration(read, id, filename, revision, scope, readImage, readFont);
  useEffect(() => {
    const requestGeneration = generation.current;
    const isCurrent = () => generation.current === requestGeneration;
    let remove: (() => void) | undefined;
    setState(read && filename ? "loading" : "idle");
    if (read && filename)
      void (async () => {
        try {
          const css = await read(id);
          if (!isCurrent()) return;
          if (css !== null) {
            const imports = await prepareToolbarImports(css, filename, async (relative) => {
              if (!isCurrent()) throw new Error("stale request");
              const imported = await read(id, relative);
              if (imported === null) throw new Error("missing import");
              return imported;
            });
            if (!isCurrent()) return;
            const fonts = readFont
              ? await prepareToolbarFonts(imports.css, async (relative) => {
                  if (!isCurrent()) throw new Error("stale request");
                  return skinFontBytes(await readFont(id, relative));
                })
              : { css: imports.css, partial: false, install: () => () => {} };
            if (!isCurrent()) return;
            const prepared = readImage
              ? await prepareToolbarImages(fonts.css, async (relative) => {
                  if (!isCurrent()) throw new Error("stale request");
                  return skinImageUrl(await readImage(id, relative));
                })
              : { css: fonts.css, partial: false };
            if (!isCurrent()) return;
            const result = installToolbarCss(scope, prepared.css);
            try {
              const removeFonts = fonts.install();
              remove = () => {
                result.remove();
                removeFonts();
              };
            } catch {
              result.remove();
              throw new Error("font installation failed");
            }
            setState(
              result.partial || prepared.partial || fonts.partial || imports.partial
                ? "partial"
                : "ready",
            );
          } else setState("ready");
        } catch {
          if (isCurrent()) setState("failed");
        }
      })();
    return () => {
      remove?.();
    };
  }, [read, id, filename, revision, scope, readImage, readFont, generation]);
  return state;
}
