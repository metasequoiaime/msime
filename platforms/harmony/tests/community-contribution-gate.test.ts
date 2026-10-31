import { CommunityContributionGate } from "../entry/src/main/ets/keyboard/CommunityContributionGate";

declare const process: { exitCode?: number };

// 鸿蒙版在实名认证到位之前，Rust 的 `HostCapabilities::for_platform(Harmony).community_contribution` 为 false；设置桥据此拒绝社区的发布、改分类、评分和举报，浏览和安装照常。

let failures = 0;
function check(condition: boolean, message: string): void {
  if (condition) {
    console.log(`  ok  ${message}`);
  } else {
    console.error(`FAIL  ${message}`);
    failures++;
  }
}

const capabilities = (contribution: boolean | undefined): string =>
  JSON.stringify({
    ok: true,
    value:
      contribution === undefined
        ? { platform: "harmony" }
        : { platform: "harmony", community_contribution: contribution },
  });

// ---- 读宿主能力 ----
check(
  !CommunityContributionGate.allowsSettingsRequest(
    "community_skin",
    JSON.stringify({ operation: "keyboard_skin_publish" }),
  ),
  "adopt 之前按关闭处理",
);
check(
  CommunityContributionGate.fromCapabilities(capabilities(false)) === false,
  "community_contribution 为 false 时关闭",
);
check(
  CommunityContributionGate.fromCapabilities(capabilities(true)) === true,
  "community_contribution 为 true 时打开",
);
check(
  CommunityContributionGate.fromCapabilities(capabilities(undefined)) === false,
  "回答里没有这一项时按关闭处理",
);
check(
  CommunityContributionGate.fromCapabilities(
    JSON.stringify({ ok: false, error: "invalid platform" }),
  ) === false,
  "宿主拒绝时按关闭处理",
);
check(CommunityContributionGate.fromCapabilities("not json") === false, "读不懂的回答按关闭处理");
check(
  CommunityContributionGate.fromCapabilities(
    JSON.stringify({ ok: true, value: { community_contribution: 1 } }),
  ) === false,
  "只有布尔 true 才算打开",
);

// ---- 总闸关着：设置桥 ----
check(CommunityContributionGate.adopt(capabilities(false)) === false, "adopt 关闭的能力后保持关闭");

const request = (operation: string, extra: Record<string, unknown> = {}): string =>
  JSON.stringify({ operation, ...extra });
const refused: [string, string][] = [
  ["community_skin", request("keyboard_skin_publish", { id: "x", name: "合成", design: {} })],
  ["community_skin", request("keyboard_skin_rate", { id: "x", stars: 5 })],
  ["community_skin", request("keyboard_skin_set_category", { id: "x", category: "cute" })],
  ["community_skin", request("report", { kind: "skins", id: "x", reason: "其他" })],
  ["community_resource", request("resource_publish", { id: "x", kind: "dictionary" })],
  ["community_resource", request("resource_rate", { id: "x", stars: 4 })],
  ["community_resource", request("report", { kind: "dictionaries", id: "x", reason: "其他" })],
  ["community_plugin", request("plugin_pack_preview", { kind: "command_table", plugin_id: "x" })],
  ["community_plugin", request("plugin_publish", { kind: "command_table", plugin_id: "x" })],
  ["community_plugin", request("plugin_rate", { id: "x", stars: 3 })],
  ["community_plugin", request("report", { kind: "plugins", id: "x", reason: "其他" })],
  // 读不出操作名的社区请求也不放行。
  ["community_skin", "not json"],
  ["community_resource", "[]"],
];
for (const [kind, payload] of refused) {
  check(
    !CommunityContributionGate.allowsSettingsRequest(kind, payload),
    `设置桥拒绝 ${kind} ${payload}`,
  );
}
const allowed: [string, string][] = [
  ["community_skin", request("keyboard_skin_list", { offset: 0, search: "" })],
  ["community_skin", request("keyboard_skin_detail", { id: "x" })],
  ["community_skin", request("keyboard_skin_install", { id: "x", name: "合成" })],
  ["community_skin", request("keyboard_skin_unpublish", { id: "x" })],
  ["community_skin", request("finish_trial", { id: "x", keep: true })],
  [
    "community_resource",
    request("resource_list", { kind: "dictionary", scope: "", search: "", offset: 0 }),
  ],
  ["community_resource", request("resource_detail", { id: "x" })],
  ["community_resource", request("resource_save", { id: "x", saved: true })],
  ["community_resource", request("resource_apply", { id: "x", resource_revision: 1 })],
  ["community_resource", request("resource_unpublish", { id: "x" })],
  ["community_resource", request("store_reply", { item: {} })],
  ["community_plugin", request("plugin_list", { offset: 0, search: "" })],
  [
    "community_plugin",
    request("plugin_install", { id: "x", kind: "command_table", plugin_id: "x" }),
  ],
  ["community_plugin", request("plugin_delete", { id: "x" })],
  // 屏蔽作者不是投稿：不公开任何内容，走三类之外的 community_block，总闸关着也照常。
  ["community_block", request("block", { kind: "skins", id: "x" })],
  ["community_block", request("block_list")],
  ["community_block", request("unblock", { id: "x" })],
  // 不是社区请求的不归这道闸管，包括同名的账号操作和反馈。
  ["account", request("report")],
  ["account", request("feedback")],
  ["account", request("rename")],
  ["cloud_dictionary", "{}"],
];
for (const [kind, payload] of allowed) {
  check(
    CommunityContributionGate.allowsSettingsRequest(kind, payload),
    `设置桥照常放行 ${kind} ${payload}`,
  );
}
check(
  JSON.parse(CommunityContributionGate.refusal()).error === "community_contribution_unavailable",
  "拒绝时给页面一个错误码，而不是让请求一直挂着",
);

// ---- 实名认证到位、总闸打开 ----
CommunityContributionGate.adopt(capabilities(true));
for (const [kind, payload] of refused.slice(0, 11)) {
  check(
    CommunityContributionGate.allowsSettingsRequest(kind, payload),
    `总闸打开时设置桥放行 ${kind} ${payload}`,
  );
}
CommunityContributionGate.adopt(capabilities(false));

if (failures > 0) process.exitCode = 1;
