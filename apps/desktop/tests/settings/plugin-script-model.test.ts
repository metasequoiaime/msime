// @vitest-environment node
import { expect, test } from "vitest";
import { runningScriptIds } from "../../../../packages/ui/src/settings/plugin-script-model";
import type {
  PluginPackage,
  ScriptGrantsResult,
} from "../../../../packages/ui/src/settings/plugin-types";

test("启用的前八项先占运行位置，之后才核对目录与内容哈希", () => {
  const packs: PluginPackage[] = Array.from({ length: 9 }, (_, index) => ({
    id: `synthetic-${index}`,
    name: `合成插件 ${index}`,
    kind: "script",
    version: "1.0.0",
    license: "CC0-1.0",
    author: null,
    description: null,
    builtin: false,
    content_hash: "synthetic-current-hash",
  }));
  const grants: ScriptGrantsResult = {
    revision: 1,
    plugins: packs.map((pack, index) => ({
      id: pack.id,
      hash: index === 0 ? "synthetic-old-hash" : pack.content_hash!,
      enabled: true,
      granted: [],
    })),
    health: [],
    commands: [],
  };

  expect(runningScriptIds(null, packs)).toEqual([]);
  expect(runningScriptIds(grants, packs)).toEqual(packs.slice(1, 8).map((pack) => pack.id));
  expect(runningScriptIds(grants, packs.slice(1))).toEqual(
    packs.slice(1, 8).map((pack) => pack.id),
  );
  grants.plugins[0].enabled = false;
  expect(runningScriptIds(grants, packs)).toEqual(packs.slice(1).map((pack) => pack.id));
});
