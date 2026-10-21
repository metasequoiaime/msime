// 生成 SDK 的内置皮肤表 `theme-catalog.js`，供 src/skin.js 导入。配色与出厂版式均在构建时从共享来源读出：
//
// - 五个全局主题（以及没有配色的 `system`、`custom`）来自 `packages/ui/src/theme/theme-catalog.json`，即 `crates/client-services/src/skin/theme.rs` 的 `catalog()` 序列化出的网页副本，Rust 测试 `web_catalog_copy_matches_the_catalog` 保证两者一致。
// - msime-windows 的内置外观（`wechat`、`graphite` 等）没有网页副本，只在 `crates/client-services/src/skin/catalog/windows_looks.rs` 的 `WINDOWS_LOOKS` 常量里，这里直接解析那张表。解析很严格：外观数、每个外观的字段、每个颜色都要对得上，`WINDOWS_LOOK_IDS` 里除没有配色的 `fluent` 外都要有表，否则直接报错，构建失败，不会悄悄生成一张缺了东西的表。
// - 八种基底的出厂版式来自 tools/factory-layouts.json，由共享 Rust 解析器生成并以测试核对；SDK 运行时不需要 TOML 解析器或宿主桥。
//
// 用法：`node packages/web-engine/tools/theme-catalog.mjs [输出文件]`，默认写到 `packages/web-engine/src/theme-catalog.js`（已被 packages/web-engine/.gitignore 忽略，直接对源码跑的测试和示例用它）。scripts/build-web-engine.sh 把它写进 npm 包；test/skin.test.mjs 每次运行前重新生成，所以测试读到的永远是当前的来源。
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
export const repoRoot = resolve(here, "../../..");
export const defaultOutput = resolve(here, "../src/theme-catalog.js");

const CATALOG = "packages/ui/src/theme/theme-catalog.json";
const WINDOWS_LOOKS = "crates/client-services/src/skin/catalog/windows_looks.rs";
const FACTORY_LAYOUTS = "packages/web-engine/tools/factory-layouts.json";
const COLOR = /^#[0-9A-F]{6}([0-9A-F]{2})?$/;
const PALETTE_SLOTS = ["surface", "border", "text", "number", "secondary", "accent", "selected", "selected_text", "selected_number", "hover"];
const LOOK_SLOTS = ["surface", "border", "text", "number", "accent", "selected", "hover"];

function fail(message) {
  throw new Error(`theme-catalog: ${message}`);
}

/** 全局主题表：每项只留 SDK 用得到的 `id`、`title`、`appearance` 和候选框配色，并核对配色都是 `#RRGGBB` 或 `#RRGGBBAA`。 */
function readThemes(root) {
  const entries = JSON.parse(readFileSync(resolve(root, CATALOG), "utf8"));
  if (!Array.isArray(entries) || entries.length === 0) fail(`${CATALOG} is not a non-empty array`);
  return entries.map((entry) => {
    if (typeof entry?.id !== "string") fail(`${CATALOG}: an entry has no id`);
    const { id, title, appearance, candidate } = entry;
    if (appearance !== null && appearance !== "light" && appearance !== "dark") fail(`${id}: bad appearance`);
    if (candidate !== null) {
      for (const slot of PALETTE_SLOTS) {
        if (!COLOR.test(candidate[slot])) fail(`${id}: candidate.${slot} is not a normalized colour`);
      }
      if (candidate.show_selected_bar !== null && typeof candidate.show_selected_bar !== "boolean") fail(`${id}: bad show_selected_bar`);
    }
    return { id, title, appearance, candidate };
  });
}

/** 解析 `LookCandidate { ... }` 的字段体：七个颜色、`show_selected_bar` 和 `selected_text`（`Some("#…")` 或 `None`），一个不多一个不少。 */
function lookPalette(id, mode, body) {
  const fields = new Map();
  for (const match of body.matchAll(/(\w+):\s*("(#[0-9A-Fa-f]+)"|Some\("(#[0-9A-Fa-f]+)"\)|None|true|false)\s*,/g)) {
    if (fields.has(match[1])) fail(`${id}.${mode}: duplicate ${match[1]}`);
    fields.set(match[1], match[3] ?? match[4] ?? (match[2] === "None" ? null : match[2] === "true"));
  }
  const palette = {};
  for (const slot of LOOK_SLOTS) {
    const value = fields.get(slot);
    if (typeof value !== "string" || !COLOR.test(value)) fail(`${id}.${mode}: ${slot} is missing or not a normalized colour`);
    palette[slot] = value;
  }
  const bar = fields.get("show_selected_bar");
  if (typeof bar !== "boolean") fail(`${id}.${mode}: show_selected_bar is missing`);
  palette.showSelectedBar = bar;
  if (!fields.has("selected_text")) fail(`${id}.${mode}: selected_text is missing`);
  const selectedText = fields.get("selected_text");
  if (selectedText !== null && (typeof selectedText !== "string" || !COLOR.test(selectedText))) fail(`${id}.${mode}: selected_text is not None or a normalized colour`);
  palette.selectedText = selectedText;
  if (fields.size !== LOOK_SLOTS.length + 2) fail(`${id}.${mode}: unexpected fields ${[...fields.keys()].join(", ")}`);
  return palette;
}

/** Windows 外观表：`WINDOWS_LOOKS` 里每个外观的 ID、候选框圆角和深浅两套候选框配色（工具栏配色 SDK 用不到）。 */
function readWindowsLooks(root) {
  const source = readFileSync(resolve(root, WINDOWS_LOOKS), "utf8");
  const table = source.match(/const WINDOWS_LOOKS: \[WindowsLook; (\d+)\] = \[([\s\S]*?)\n\];/);
  if (!table) fail(`${WINDOWS_LOOKS}: WINDOWS_LOOKS not found`);
  const ids = source.match(/pub const WINDOWS_LOOK_IDS: \[&str; (\d+)\] = \[([\s\S]*?)\];/);
  if (!ids) fail(`${WINDOWS_LOOKS}: WINDOWS_LOOK_IDS not found`);
  const lookIds = [...ids[2].matchAll(/"([a-z_]+)"/g)].map((match) => match[1]);
  if (lookIds.length !== Number(ids[1])) fail(`${WINDOWS_LOOKS}: WINDOWS_LOOK_IDS has ${lookIds.length} ids, declared ${ids[1]}`);
  const looks = [];
  const pattern = /WindowsLook \{\s*id: "([a-z_]+)",\s*corner_radius_dip: (\d+(?:\.\d+)?),\s*toolbar_corner_radius_dip: [\d.]+,\s*dark: LookCandidate \{([^}]*)\},\s*light: LookCandidate \{([^}]*)\},/g;
  for (const match of table[2].matchAll(pattern)) {
    const [, id, radius, dark, light] = match;
    looks.push({ id, cornerRadiusDip: Number(radius), dark: lookPalette(id, "dark", dark), light: lookPalette(id, "light", light) });
  }
  const declared = (table[2].match(/WindowsLook \{/g) ?? []).length;
  if (looks.length !== Number(table[1]) || declared !== looks.length) fail(`${WINDOWS_LOOKS}: parsed ${looks.length} of ${table[1]} looks (${declared} literals); the table layout changed, update this parser`);
  // `WINDOWS_LOOK_IDS` 里没有配色表的外观就是 `system` 本身（目前只有 `fluent`），清单 `base` 写它时什么也不补。
  const plain = lookIds.filter((id) => !looks.some((look) => look.id === id));
  for (const look of looks) if (!lookIds.includes(look.id)) fail(`${look.id} is in WINDOWS_LOOKS but not in WINDOWS_LOOK_IDS`);
  const canvas = source.match(/const CANVAS_LOOK: WindowsLook = WindowsLook \{[\s\S]*?dark: LookCandidate \{([^}]*)\},\s*light: LookCandidate \{([^}]*)\},/);
  if (!canvas) fail(`${WINDOWS_LOOKS}: CANVAS_LOOK not found`);
  const canvasLook = { dark: lookPalette("canvas", "dark", canvas[1]), light: lookPalette("canvas", "light", canvas[2]) };
  for (const mode of ["dark", "light"]) {
    const detail = source.match(new RegExp(`const CANVAS_DETAIL_${mode.toUpperCase()}: LookDetail = LookDetail \\{([\\s\\S]*?)\\n\\};`));
    const translation = detail?.[1].match(/translation: Some\("(#[0-9A-F]+)"\)/)?.[1];
    if (!COLOR.test(translation ?? "")) fail(`canvas.${mode}: translation is missing`);
    canvasLook[mode].translation = translation;
  }
  return { looks, plain, canvasLook };
}

/** 生成的模块源码。输出只取决于共享来源文件，同一提交生成的内容逐字节相同。 */
export function themeCatalogModule(root = repoRoot) {
  const themes = readThemes(root);
  const { looks, plain, canvasLook } = readWindowsLooks(root);
  const factoryLayouts = JSON.parse(readFileSync(resolve(root, FACTORY_LAYOUTS), "utf8"));
  const factoryIds = [...looks.map((look) => look.id), ...plain, "canvas"].sort();
  if (JSON.stringify(Object.keys(factoryLayouts).sort()) !== JSON.stringify(factoryIds)) fail(`${FACTORY_LAYOUTS}: factory IDs do not match Windows bases`);
  for (const id of factoryIds) {
    if (!["geometry", "horizontal", "vertical"].every((key) => factoryLayouts[id]?.[key] && typeof factoryLayouts[id][key] === "object")) fail(`${id}: missing factory geometry`);
  }
  for (const look of [...looks.map((look) => look.id), ...plain]) {
    if (themes.some((theme) => theme.id === look)) fail(`${look} is both a global theme and a Windows look`);
  }
  return [
    `// 由 packages/web-engine/tools/theme-catalog.mjs 从 ${CATALOG}、${WINDOWS_LOOKS} 和 ${FACTORY_LAYOUTS} 生成，不要手改。`,
    `export const themes = ${JSON.stringify(themes)};`,
    `export const windowsLooks = ${JSON.stringify(looks)};`,
    `export const canvasLook = ${JSON.stringify(canvasLook)};`,
    `export const plainWindowsLooks = ${JSON.stringify(plain)};`,
    `export const factoryLayouts = ${JSON.stringify(factoryLayouts)};`,
    "",
  ].join("\n");
}

export function writeThemeCatalog(output = defaultOutput, root = repoRoot) {
  writeFileSync(output, themeCatalogModule(root));
  return output;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  writeThemeCatalog(process.argv[2] ? resolve(process.argv[2]) : defaultOutput);
}
