// @vitest-environment jsdom
// macOS 液态玻璃下，正文卡片的底色（`bg-raised`、`bg-card`、`--dropdown-bg`）被换成薄透明色。浮在页面上的层如果沿用这些底色，背后的皮肤预览、看板娘和分页按钮就会透上来。这里守住约定：每个浮层都带 `settings-modal-surface`，样式表里对应的厚玻璃规则也在。
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { useEffect, type ReactNode } from "react";
import { CandidateSkinPublishDialog, CommunityResourceScopeButtons, useConfirm } from "@msime/ui";
import * as accountStyle from "../../../../packages/ui/src/account/account-style";
import * as communityStyle from "../../../../packages/ui/src/community/community-style";
import * as fullScreenStyle from "../../../../packages/ui/src/core/full-screen-style";
import stylesheet from "../../../../packages/ui/src/styles.css?raw";

const uiPrefix = "../../../../packages/ui/src/";
// 共享界面的全部组件源码，键是相对 `packages/ui/src` 的路径。
const sources = Object.fromEntries(
  Object.entries(
    import.meta.glob<string>(["../../../../packages/ui/src/**/*.tsx", "!**/*.test.tsx"], {
      query: "?raw",
      import: "default",
      eager: true,
    }),
  ).map(([path, text]) => [path.slice(uiPrefix.length), text]),
);
const surfaceClass = "settings-modal-surface";
const surfaceSelector = `html[data-macos-liquid-glass="true"] [data-settings-shell][data-platform="mac"] .${surfaceClass}`;

// 浮层的 className 引用到的样式模块。新的浮层用了别的样式模块，就在这里登记，测试才能解析它的类名。
const styleModules: Record<string, Record<string, unknown>> = {
  "account/account-style.ts": accountStyle,
  "community/community-style.ts": communityStyle,
  "core/full-screen-style.ts": fullScreenStyle,
};

// 不带厚玻璃的 `aria-modal` 面板，各自写明为什么在 macOS 设置窗口的玻璃下不会透出背后的页面。`count` 是该文件里 `aria-modal="true"` 的个数，新增一个就要重新判断。
const modalExemptions: Record<string, { count: number; reason: string }> = {
  "core/action-sheet.tsx": {
    count: 1,
    reason: "HarmonyOS 和手机设置的底部选择面板，macOS 设置窗口不渲染它",
  },
  "keyboard/keyboard-switch-guide.tsx": {
    count: 1,
    reason: "iOS 和 Android 设为默认输入法的引导，只在手机上打开",
  },
  "keyboard/panels.tsx": {
    count: 1,
    reason: "键盘面板窗口里的确认，不在设置外壳 `[data-settings-shell]` 里，玻璃规则不作用于它",
  },
  "settings/uninstall-section.tsx": {
    count: 1,
    reason: "卸载确认是页面流里的卡片，下面没有别的内容，背后只有窗口材质，与其他分组卡片同一层",
  },
};

// 弹出列表与菜单。它们不用正文卡片的底色，或者不浮在页面上，各自写明理由；新增一个就要重新判断。
const popupExemptions: Record<string, string> = {
  "keyboard/emoji-clipboard-entries.tsx":
    "剪贴板记录的右键菜单在表情面板自己的窗口里，用面板配色，不在设置页的玻璃外壳里",
  "candidate/font-family-input.tsx":
    "`.font-family-menu` 使用近乎不透明的 `--dropdown-menu-bg`，玻璃外壳不改它",
  "settings/ai-provider-select.tsx":
    "`.ai-provider-menu` 自带厚玻璃规则与减少透明度、强制颜色的回退",
  "settings/statistics-options-menu.tsx": "统计页的溢出菜单只在 HarmonyOS 和手机外观里出现",
  "settings/use-settings-shell-navigation.tsx": "搜索结果就地替换侧栏列表，不浮在页面上",
};

/** 把组件里的相对导入解析成相对 `packages/ui/src` 的 `.ts` 路径。 */
function importedPath(file: string, from: string): string {
  const parts = file.split("/").slice(0, -1);
  for (const part of from.split("/")) {
    if (part === "..") parts.pop();
    else if (part !== ".") parts.push(part);
  }
  return `${parts.join("/")}.ts`;
}

/** 从属性所在位置往回找到 JSX 开始标签，再按花括号深度找到标签结束，返回整个开始标签。 */
function openingTag(text: string, attribute: number): string {
  let start = text.lastIndexOf("<", attribute);
  while (start > 0 && !/[A-Za-z]/.test(text[start + 1])) start = text.lastIndexOf("<", start - 1);
  let depth = 0;
  for (let index = start + 1; index < text.length; index++) {
    const char = text[index];
    if (char === "{") depth++;
    else if (char === "}") depth--;
    else if (depth === 0 && char === '"') index = text.indexOf('"', index + 1);
    else if (depth === 0 && char === ">") return text.slice(start, index + 1);
  }
  throw new Error("unterminated JSX tag");
}

/** 开始标签里 `className` 的原文：字符串字面量，或花括号里的整个表达式。 */
function classNameSource(tag: string): string | null {
  const at = tag.indexOf("className=");
  if (at < 0) return null;
  const value = at + "className=".length;
  if (tag[value] === '"') return tag.slice(value + 1, tag.indexOf('"', value + 1));
  let depth = 0;
  for (let index = value; index < tag.length; index++) {
    if (tag[index] === "{") depth++;
    else if (tag[index] === "}" && --depth === 0) return tag.slice(value + 1, index);
  }
  return null;
}

/** 把 className 表达式里引用的样式常量换成它们的值；引用了未登记样式模块的面板解析不出这个类，会被报为缺少厚玻璃。 */
function resolvedClasses(file: string, text: string, source: string): string {
  const scope = new Map<string, unknown>();
  for (const [, alias, from] of text.matchAll(/import \* as (\w+) from "(\.[^"]+)"/g)) {
    const module = styleModules[importedPath(file, from)];
    if (module) scope.set(alias, module);
  }
  for (const [, names, from] of text.matchAll(/import \{([^}]+)\} from "(\.[^"]+)"/g)) {
    const module = styleModules[importedPath(file, from)];
    if (!module) continue;
    for (const name of names.split(",").map((part) => part.trim()))
      if (name in module) scope.set(name, module[name]);
  }
  const values = [source];
  for (const [, owner, member] of source.matchAll(/\b(\w+)\.(\w+)\b/g)) {
    const module = scope.get(owner) as Record<string, unknown> | undefined;
    if (!module) continue;
    expect(module[member], `${file}: ${owner}.${member}`).toBeTypeOf("string");
    values.push(module[member] as string);
  }
  for (const [name] of source.matchAll(/\b\w+\b/g)) {
    const value = scope.get(name);
    if (typeof value === "string") values.push(value);
  }
  return values.join(" ");
}

test("样式表为浮层提供厚玻璃，并在减少透明度和强制颜色下回退", () => {
  const css = stylesheet.replace(/\s+/g, " ");
  const selector = surfaceSelector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  // 规则体按出现的上下文区分：紧跟在 `@media (...) {` 后面的是该媒体查询里的回退，否则是顶层规则。
  const rule = (context: string) => {
    const match = css.match(new RegExp(`${context}${selector} \\{([^}]*)\\}`));
    expect(match, `missing modal surface rule after ${context}`).not.toBeNull();
    return match![1];
  };
  const base = rule("(?<!\\{ )");
  expect(base).toContain("background: color-mix(in srgb, var(--p-bg) 90%, transparent)");
  expect(base).toContain("backdrop-filter: blur(");
  const reduced = rule("@media \\(prefers-reduced-transparency: reduce\\) \\{ ");
  expect(reduced).toContain("background: var(--p-bg)");
  expect(reduced).toContain("backdrop-filter: none");
  const forced = rule("@media \\(forced-colors: active\\) \\{ ");
  expect(forced).toContain("background: Canvas");
  expect(forced).toContain("color: CanvasText");
});

test("每个 aria-modal 面板都带厚玻璃表面，豁免项写明理由且与源码一致", () => {
  const counted = new Map<string, number>();
  const missing: string[] = [];
  for (const [file, text] of Object.entries(sources)) {
    for (const match of text.matchAll(/aria-modal="true"/g)) {
      counted.set(file, (counted.get(file) ?? 0) + 1);
      if (modalExemptions[file]) continue;
      const tag = openingTag(text, match.index!);
      const source = classNameSource(tag);
      if (!source || !resolvedClasses(file, text, source).split(/\s+/).includes(surfaceClass))
        missing.push(`${file}: ${tag.split("\n")[0]} className=${source}`);
    }
  }
  expect(missing).toEqual([]);
  for (const [file, { count }] of Object.entries(modalExemptions))
    expect(counted.get(file) ?? 0, `${file} 的 aria-modal 个数变了`).toBe(count);
  // 有浮层才谈得上覆盖；这个数字掉到零说明扫描本身坏了。
  expect([...counted.keys()].filter((file) => !modalExemptions[file]).length).toBeGreaterThan(5);
});

test("弹出列表与菜单逐个登记", () => {
  const popups = Object.entries(sources)
    .filter(([, text]) => /role="(menu|listbox)"/.test(text))
    .map(([file]) => file)
    .sort();
  expect(popups).toEqual(Object.keys(popupExemptions).sort());
});

function GlassShell({ children }: { children: ReactNode }) {
  return (
    <div data-settings-shell="" data-platform="mac">
      {children}
    </div>
  );
}

function OpenConfirm() {
  const { confirm, confirmation } = useConfirm();
  useEffect(() => {
    void confirm({ message: "删除合成皮肤？", danger: true });
  }, [confirm]);
  return confirmation;
}

beforeEach(() => document.documentElement.setAttribute("data-macos-liquid-glass", "true"));
afterEach(() => {
  cleanup();
  document.documentElement.removeAttribute("data-macos-liquid-glass");
});

test("macOS 玻璃外壳里的发布对话框、确认框和筛选菜单命中厚玻璃规则", async () => {
  render(
    <GlassShell>
      <CandidateSkinPublishDialog
        client={{
          packPreview: vi.fn(),
          addPreview: vi.fn(),
          addLicense: vi.fn(),
          publish: vi.fn(),
        }}
        onClose={vi.fn()}
        onPublished={vi.fn()}
      />
      <OpenConfirm />
      <CommunityResourceScopeButtons resourceLabel="皮肤" scope="" onScopeChange={vi.fn()} />
    </GlassShell>,
  );
  const surfaces = [
    screen.getByRole("dialog", { name: "发布候选窗口皮肤" }),
    await screen.findByRole("alertdialog"),
    screen.getByRole("group", { name: "皮肤筛选范围", hidden: true }),
  ];
  for (const surface of surfaces)
    expect(surface.matches(surfaceSelector), surface.getAttribute("aria-label") ?? "").toBe(true);
});
