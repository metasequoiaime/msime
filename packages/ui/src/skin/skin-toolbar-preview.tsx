import toolbarMarkup from "../upstream/skin-toolbar-preview.html?raw";
import { useEffect, useRef, type CSSProperties } from "react";
import type { FloatingToolbarPreferences } from "../settings/floating-toolbar-preferences";

/** Build-time, script-free upstream sample; never accepts runtime HTML. */
export function SkinToolbarPreview({ preferences }: { preferences?: FloatingToolbarPreferences }) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const language = host.current?.querySelector<HTMLElement>('[data-toolbar-item="language"]');
    if (language)
      language.style.display = !preferences || preferences.english_mode ? "flex" : "none";
    for (const item of [
      "fullwidth",
      "punctuation",
      "character_set",
      "emoji",
      "screen_keyboard",
      "settings",
    ] as const) {
      const element = host.current?.querySelector<HTMLElement>(`[data-toolbar-item="${item}"]`);
      if (element) element.style.display = !preferences || preferences[item] ? "flex" : "none";
    }
  }, [preferences]);
  const style = preferences
    ? ({
        "--ftb-scale": preferences.scale_percent / 100,
        "--ftb-icon-size": `${preferences.font_size}px`,
      } as CSSProperties)
    : undefined;
  return (
    <div
      ref={host}
      className="ftb-preview-host"
      style={style}
      aria-hidden="true"
      dangerouslySetInnerHTML={{ __html: toolbarMarkup }}
    />
  );
}
