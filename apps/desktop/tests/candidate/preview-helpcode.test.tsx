// @vitest-environment jsdom
import { afterEach, expect, test } from "vitest";
import { cleanup, render, waitFor } from "@testing-library/react";
import { AppearanceCandidatePreview } from "../../../../packages/ui/src/candidate/appearance-candidate-preview";
import type { Preferences, SkinCatalog } from "@msime/ui";

afterEach(cleanup);
const preferences: Preferences = {
  scheme: "quanpin",
  shuangpin_profile: "xiaohe",
  candidate_page_size: 6,
  learning: true,
  chinese_punctuation: true,
};
const catalog: SkinCatalog = {
  directory: "/synthetic/skins",
  issues: [],
  packages: [
    {
      id: "sample",
      name: "Sample",
      version: "1",
      base: "system",
      author: null,
      description: null,
      layouts: ["horizontal", "vertical"],
      themes: ["dark"],
      minWidthDip: 0,
      decorationTopDip: 0,
      decorationWidthDip: 0,
      toolbarStylesheet: null,
      preview: null,
      candidate: { dark: {}, light: {} },
    },
  ],
};
const scan = async () => catalog;

const selections: Record<string, Pick<Preferences, "global_theme" | "custom_theme">> = {
  system: { global_theme: "system" },
  shuishan: { global_theme: "shuishan" },
  light: { global_theme: "light" },
  paper: { global_theme: "paper" },
  night: { global_theme: "night" },
  ink: { global_theme: "ink" },
  "custom colours": {
    global_theme: "custom",
    custom_theme: { candidate_colors: { text: "#112233" } },
  },
  "custom package": { global_theme: "custom", custom_theme: { candidate_skin: "sample" } },
};

test.each(Object.keys(selections))(
  "%s preview follows each scheme's enable and display flags",
  async (name) => {
    const selection = selections[name];
    const view = render(
      <AppearanceCandidatePreview preferences={{ ...preferences, ...selection }} scan={scan} />,
    );
    await waitFor(() => expect(view.container.querySelectorAll(".cand")).toHaveLength(6));
    for (const candidate_layout of ["horizontal", "vertical"] as const) {
      for (const scheme of ["quanpin", "shuangpin"] as const) {
        for (const enabled of [false, true]) {
          for (const show_in_candidate_window of [false, true, undefined]) {
            view.rerender(
              <AppearanceCandidatePreview
                preferences={{
                  ...preferences,
                  ...selection,
                  candidate_layout,
                  scheme,
                  quanpin_helpcode: {
                    enabled: false,
                    schema: "ziranma",
                    show_in_candidate_window: false,
                  },
                  shuangpin_helpcode: {
                    enabled: false,
                    schema: "ziranma",
                    show_in_candidate_window: false,
                  },
                  [`${scheme}_helpcode`]: { enabled, schema: "ziranma", show_in_candidate_window },
                }}
                scan={scan}
              />,
            );
            expect(view.container.querySelectorAll(".cand-helpcode")).toHaveLength(
              enabled && show_in_candidate_window !== false ? 6 : 0,
            );
            expect(view.container.querySelectorAll(".cand")).toHaveLength(6);
            expect(view.container.querySelector(".pinyin")).not.toBeNull();
          }
        }
      }
    }
    for (const scheme of ["wubi", "japanese"] as const) {
      view.rerender(
        <AppearanceCandidatePreview
          preferences={{
            ...preferences,
            ...selection,
            scheme,
            quanpin_helpcode: { enabled: true, schema: "ziranma", show_in_candidate_window: true },
            shuangpin_helpcode: {
              enabled: true,
              schema: "ziranma",
              show_in_candidate_window: true,
            },
          }}
          scan={scan}
        />,
      );
      expect(view.container.querySelectorAll(".cand-helpcode")).toHaveLength(0);
    }
  },
);

test.each([
  ["quanpin", 0],
  ["shuangpin", 6],
] as const)(
  "%s preview without a stored helpcode object follows the core default",
  async (scheme, shown) => {
    const view = render(
      <AppearanceCandidatePreview preferences={{ ...preferences, scheme }} scan={scan} />,
    );
    await waitFor(() => expect(view.container.querySelectorAll(".cand")).toHaveLength(6));
    expect(view.container.querySelectorAll(".cand-helpcode")).toHaveLength(shown);
  },
);
