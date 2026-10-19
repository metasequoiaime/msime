/**
 * 鸿蒙宿主交给 Rust 账号入口的请求（#8201 第 2 步）与两个沙箱之间的会话镜像规则。NAPI 和文件不在这里，只检查宿主拼出的文档、放行的操作和采用的先后。
 */
import {
  PAGE_ACCOUNT_OPERATIONS,
  SESSION_CHANGING_OPERATIONS,
  accountRequest,
  aiSkinRequest,
  anonymousAccountRequest,
  appliedFeedbackText,
  clientAgentRequest,
  cloudClipboardDeviceName,
  cloudClipboardRequest,
  cloudDictionaryFileRequest,
  cloudDictionaryRequest,
  communityRequest,
  hostAccountRequest,
  preferencesPutAction,
  replyError,
  replyValueText,
  settingsApplyRequest,
  settingsExportRequest,
  statusUserId,
  typingUploadReply,
  typingUploadRequest,
} from "../entry/src/main/ets/account/HarmonyAccountRequests";
import {
  ADOPTED_SESSIONS,
  AdoptionGate,
  AdoptionTracker,
  SerialGate,
  adoptionFailureMessage,
  adoptionResult,
  adoptionFingerprint,
  sessionAdoptRequest,
} from "../entry/src/main/ets/account/AccountSessionAdoptionPolicy";
import {
  KEYBOARD_ENTRIES,
  SETTINGS_ENTRIES,
  SharedStateMirrorPolicy,
} from "../entry/src/main/ets/keyboard/SharedStateMirrorPolicy";

let failures = 0;
let checks = 0;

async function group(name: string, body: () => void | Promise<void>): Promise<void> {
  try {
    await body();
    console.log(`  ok  ${name}`);
  } catch (error) {
    failures++;
    console.log(`FAIL  ${name}`);
    console.log(`      ${error instanceof Error ? error.message : String(error)}`);
  }
}

function check(condition: boolean, message: string): void {
  checks++;
  if (!condition) throw new Error(message);
}

type Json = Record<string, unknown>;

function parsed(text: string): Json {
  return JSON.parse(text) as Json;
}

const DIRECTORY = "/data/storage/el2/base/haps/entry/files/state";
const AGENT = "msime-harmony/0.1.0 (HUAWEI Mate 70; HarmonyOS 6.0.1.115; edition=full)";

async function main(): Promise<void> {
  console.log("Harmony account requests");

  await group(
    "page account operations go to Rust with the state directory and the client agent",
    () => {
      const plan = accountRequest(DIRECTORY, AGENT, '{"operation":"status"}');
      check(plan.refusal === "", "status is a page operation");
      const request = parsed(plan.request);
      check(request.directory === DIRECTORY, "the directory is the host's");
      check(request.client_agent === AGENT, "the client agent travels with every account call");
      check((request.action as Json).operation === "status", "the action is forwarded");
      const anonymous = parsed(accountRequest(DIRECTORY, "", '{"operation":"status"}').request);
      check(
        !("client_agent" in anonymous),
        "an empty agent is left out rather than sent as an invalid one",
      );
      for (const operation of ["account_delete", "forget", "chat_models", "qq_links", "avatar"]) {
        check(PAGE_ACCOUNT_OPERATIONS.includes(operation), `${operation} is offered to the page`);
      }
    },
  );

  await group(
    "operations that carry paths or are not on the shared page are refused before Rust",
    () => {
      for (const operation of [
        "upload_avatar",
        "data_export",
        "preferences_upload",
        "preferences_apply",
        "diagnostics_upload",
        "typing_upload_set",
        "session_adopt",
        "ensure_anonymous",
        "delete",
        "google_login",
        "voice_contribution",
      ]) {
        const plan = accountRequest(
          DIRECTORY,
          AGENT,
          JSON.stringify({ operation, path: "/etc/passwd" }),
        );
        check(plan.request === "", `${operation} never reaches Rust from the page`);
        check(
          replyError(plan.refusal, "") === "account_invalid",
          `${operation} is account_invalid`,
        );
      }
      check(
        accountRequest(DIRECTORY, AGENT, "not json").refusal.includes("account_invalid"),
        "malformed documents are refused",
      );
    },
  );

  await group("feedback is rebuilt by the host: no screenshots, always harmony", () => {
    const plan = accountRequest(
      DIRECTORY,
      AGENT,
      JSON.stringify({
        operation: "feedback",
        type: "bug",
        text: "候选不见了",
        platform: "windows",
        app_version: "0.1.0",
        edition: "full",
        diagnostics: { device: "Mate 70" },
        screenshots: ["/data/storage/el2/base/secret.png"],
      }),
    );
    const action = parsed(plan.request).action as Json;
    check(action.platform === "harmony", "the platform is this host's, not the page's");
    check(!("screenshots" in action), "a page cannot name a file for Rust to read");
    check((action.diagnostics as Json).device === "Mate 70", "diagnostics are kept");
    const bare = parsed(
      accountRequest(
        DIRECTORY,
        AGENT,
        JSON.stringify({
          operation: "feedback",
          type: "bug",
          text: "x",
          app_version: "1",
          edition: "full",
          diagnostics: null,
        }),
      ).request,
    ).action as Json;
    check(!("diagnostics" in bare), "null diagnostics are left out, Rust only takes an object");
  });

  await group("session-changing operations include the host's Google loopback login", () => {
    for (const operation of ["login", "logout", "account_delete", "forget", "google_sign_in"]) {
      check(
        SESSION_CHANGING_OPERATIONS.includes(operation),
        `${operation} cancels the old account's queued snapshot`,
      );
    }
    check(!SESSION_CHANGING_OPERATIONS.includes("status"), "reads do not");
  });

  await group("host account requests are composed from an action document", () => {
    const request = hostAccountRequest(
      DIRECTORY,
      AGENT,
      '{"operation":"google_challenge","target":"http://127.0.0.1:50000/callback"}',
    );
    check(
      request !== null && (parsed(request).action as Json).operation === "google_challenge",
      "the action is wrapped",
    );
    check(
      hostAccountRequest(DIRECTORY, AGENT, "[]") === null,
      "a document without an operation is not sent",
    );
  });

  await group("community: only the shared page's operations, install gets the state root", () => {
    const install = communityRequest(
      DIRECTORY,
      DIRECTORY,
      '{"operation":"keyboard_skin_install","id":"x","name":"秋"}',
    );
    check(
      parsed(install.request).state_root === DIRECTORY,
      "installing writes the trial and the library",
    );
    const list = parsed(
      communityRequest(
        DIRECTORY,
        DIRECTORY,
        '{"operation":"keyboard_skin_list","offset":0,"search":""}',
      ).request,
    );
    check(!("state_root" in list), "listing needs no state root");
    for (const operation of [
      "finish_trial",
      "store_reply",
      "candidate_skin_install",
      "plugin_install",
    ]) {
      const plan = communityRequest(DIRECTORY, DIRECTORY, JSON.stringify({ operation }));
      check(replyError(plan.refusal, "") === "community_invalid", `${operation} is not forwarded`);
    }
  });

  await group("cloud dictionary: short requests pass, file operations are the host's", () => {
    check(
      cloudDictionaryRequest(
        DIRECTORY,
        '{"operation":"list","kind":"pinyin","offset":0,"search":""}',
      ).refusal === "",
      "list passes",
    );
    for (const operation of [
      "export_file",
      "snapshot_export_file",
      "snapshot_restore_file",
      "snapshot_status",
      "export",
    ]) {
      const plan = cloudDictionaryRequest(DIRECTORY, JSON.stringify({ operation, path: "/tmp/x" }));
      check(
        replyError(plan.refusal, "") === "invalid_cloud_dictionary",
        `${operation} is refused with the cloud dictionary code`,
      );
    }
    const file = cloudDictionaryFileRequest(
      DIRECTORY,
      `${DIRECTORY}/cloud-dictionary-transfer`,
      '{"user_data":"/s/user"}',
      '{"operation":"snapshot_status"}',
    );
    const request = parsed(file ?? "{}");
    check(
      request.snapshot_directory === `${DIRECTORY}/cloud-dictionary-transfer`,
      "the transfer directory is private to settings",
    );
    check(
      (request.options as Json).user_data === "/s/user",
      "HostOptions are passed as a document",
    );
    const withoutOptions = parsed(
      cloudDictionaryFileRequest(DIRECTORY, "/t", null, '{"operation":"export_file"}') ?? "{}",
    );
    check(!("options" in withoutOptions), "file requests that need no options do not carry them");
  });

  await group(
    "cloud clipboard: the device name always travels, panel-less actions are refused",
    () => {
      const plan = cloudClipboardRequest(
        DIRECTORY,
        "HUAWEI Mate 70",
        '{"operation":"list","search":""}',
      );
      check(
        parsed(plan.request).device_name === "HUAWEI Mate 70",
        "the device name is sent so other devices do not see Linux",
      );
      for (const operation of ["pin", "set_retention", "clear"]) {
        check(
          replyError(
            cloudClipboardRequest(DIRECTORY, "x", JSON.stringify({ operation })).refusal,
            "",
          ) === "invalid_cloud_clipboard",
          `${operation} has no entry on this host's page yet`,
        );
      }
      check(
        cloudClipboardDeviceName(" HUAWEI Mate 70 ", "ALN-AL00") === "HUAWEI Mate 70",
        "the market name comes first",
      );
      check(cloudClipboardDeviceName("", " ALN-AL00 ") === "ALN-AL00", "then the model");
      check(cloudClipboardDeviceName(" ", "") === "HarmonyOS 设备", "then the old fallback");
    },
  );

  await group("the client agent request names harmony and the device", () => {
    const request = parsed(clientAgentRequest("0.1.0", "", "ALN-AL00", "6.0.1.115", "full"));
    check(request.platform === "harmony", "platform");
    check(request.device === "ALN-AL00", "the model stands in for a missing market name");
    check(request.system === "HarmonyOS 6.0.1.115", "system and release");
    check(
      parsed(clientAgentRequest("1", "M", "", null, "full")).system === "HarmonyOS",
      "no release, no number",
    );
    check(
      parsed(anonymousAccountRequest(DIRECTORY, AGENT)).client_agent === AGENT,
      "anonymous registration reports the agent",
    );
  });

  await group("typing upload maps the page's three actions onto typing_upload_*", () => {
    const status = parsed(
      typingUploadRequest(DIRECTORY, AGENT, DIRECTORY, "2026-10-17", '{"operation":"status"}')
        .request,
    ).action as Json;
    check(
      status.operation === "typing_upload_status" && status.statistics_directory === DIRECTORY,
      "status",
    );
    const set = parsed(
      typingUploadRequest(
        DIRECTORY,
        AGENT,
        DIRECTORY,
        "2026-10-17",
        '{"operation":"set_enabled","enabled":true}',
      ).request,
    ).action as Json;
    check(
      set.operation === "typing_upload_set" &&
        set.enabled === true &&
        set.day === "2026-10-17" &&
        set.platform === "harmony",
      "set",
    );
    const sync = parsed(
      typingUploadRequest(DIRECTORY, AGENT, DIRECTORY, "2026-10-17", '{"operation":"sync"}')
        .request,
    ).action as Json;
    check(
      sync.operation === "typing_upload" && !("force" in sync),
      "sync waits out the ten minutes",
    );
    check(
      typingUploadRequest(DIRECTORY, AGENT, DIRECTORY, "d", '{"operation":"set_enabled"}')
        .request === "",
      "set needs a boolean",
    );
    const reply = parsed(
      typingUploadReply(
        '{"ok":true,"value":{"enabled":true,"last_upload_ms":null,"signed_in":false}}',
      ),
    );
    const value = reply.value as Json;
    check(
      value.enabled === true && value.lastUploadMs === null && value.signedIn === false,
      "the page reads camelCase",
    );
    check(
      typingUploadReply('{"ok":false,"error":"account_network"}').includes("account_network"),
      "failures pass through",
    );
  });

  await group("AI skin generate and cancel", () => {
    const generate = parsed(
      aiSkinRequest(DIRECTORY, '{"operation":"generate","request_id":"r1","prompt":"秋天"}')
        .request,
    );
    check(
      generate.directory === DIRECTORY &&
        generate.request_id === "r1" &&
        generate.prompt === "秋天",
      "generate",
    );
    const cancel = parsed(
      aiSkinRequest(DIRECTORY, '{"operation":"cancel","request_id":"r1"}').request,
    );
    check(!("directory" in cancel) && cancel.request_id === "r1", "cancel only names the run");
    check(
      replyError(aiSkinRequest(DIRECTORY, '{"operation":"generate"}').refusal, "") ===
        "ai_skin_invalid",
      "a run needs an id",
    );
  });

  await group("replies", () => {
    check(
      statusUserId('{"ok":true,"value":{"user":{"id":"u1","displayName":"a"}}}') === "u1",
      "the signed-in id",
    );
    check(statusUserId('{"ok":true,"value":{"user":null}}') === null, "signed out");
    check(statusUserId('{"ok":false,"error":"account_storage"}') === null, "unreadable");
    check(replyValueText('{"ok":true,"value":null}') === "null", "a null value is still a value");
    check(replyValueText('{"ok":false,"error":"x"}') === null, "failure has none");
  });

  await group("settings sync: export, put bound to the account, apply always with feedback", () => {
    const feedback = '{"soundEnabled":true,"hapticsEnabled":false,"hapticStrength":"strong"}';
    const schema = '{"fields":{},"maximumBytes":1,"updateMode":"replace","revisionRequired":true}';
    const cloud = '{"revision":3,"settings":{"input.schema":"wubi"}}';
    const exported = parsed(settingsExportRequest("/s", feedback, schema, cloud) ?? "{}");
    check((exported.feedback as Json).hapticStrength === "strong", "feedback is in the export");
    check((exported.cloud as Json).revision === 3, "the cloud document is merged into");
    const put = parsed(
      preferencesPutAction(
        '{"settings":{},"merged":{"revision":3,"settings":{"input.schema":"quanpin"}}}',
        "u1",
      ) ?? "{}",
    );
    check(
      put.operation === "preferences_put" && put.revision === 3 && put.user_id === "u1",
      "the upload is compare-and-swap and bound to the account",
    );
    check(preferencesPutAction('{"settings":{}}', "u1") === null, "no merged document, no upload");
    const apply = parsed(settingsApplyRequest("/s", cloud, schema, feedback) ?? "{}");
    check(
      (apply.feedback as Json).soundEnabled === true,
      "apply always carries the feedback (difference 97)",
    );
    check(
      settingsApplyRequest("/s", cloud, schema, "null") === null,
      "missing feedback is not sent",
    );
    check(
      appliedFeedbackText('{"ok":true,"value":{"feedback":null}}') === null,
      "nothing to write back",
    );
    check(
      appliedFeedbackText('{"ok":true,"value":{"feedback":{"soundEnabled":false}}}') ===
        '{"soundEnabled":false}',
      "feedback to write back",
    );
  });

  console.log("Harmony session mirror");

  await group("both sessions and the anonymous identity are adopted by Rust", () => {
    for (const name of ["account-session", "anonymous-session", "anonymous-account"]) {
      check(
        SharedStateMirrorPolicy.adoptedByRust(name),
        `${name} is decided by session_adopt, not overwritten`,
      );
    }
    for (const name of ["preferences", "typing-statistics"]) {
      check(!SharedStateMirrorPolicy.adoptedByRust(name), `${name} is still mirrored as before`);
    }
    for (const session of ADOPTED_SESSIONS) {
      for (const name of [session.entry, session.identityEntry]) {
        if (name === null) continue;
        check(
          SETTINGS_ENTRIES.some((entry) => entry.name === name),
          `settings publishes ${name}`,
        );
        check(
          KEYBOARD_ENTRIES.some((entry) => entry.name === name),
          `the keyboard publishes ${name}`,
        );
      }
    }
    const identities = ADOPTED_SESSIONS.map(
      (session) => `${session.item}:${session.identityEntry}`,
    );
    check(
      identities.join(",") === "account:null,anonymous:anonymous-account",
      `only the anonymous session carries the identity (got ${identities.join(",")})`,
    );
  });

  await group("the anonymous identity travels with the anonymous session", () => {
    const action = parsed(
      sessionAdoptRequest(
        DIRECTORY,
        "/group/msime/keyboard/state/anonymous-session.json",
        "anonymous",
        "keyboard",
        "/group/msime/keyboard/state/anonymous-account.json",
      ),
    ).action as Json;
    check(
      action.identity_path === "/group/msime/keyboard/state/anonymous-account.json",
      "the published identity file goes with the anonymous session",
    );
    const account = parsed(
      sessionAdoptRequest(
        DIRECTORY,
        "/group/msime/settings/state/account-session.json",
        "account",
        "settings",
      ),
    ).action as Json;
    check(!("identity_path" in account), "the login session carries no identity");
    check(
      adoptionFingerprint("s1", "absent") !== adoptionFingerprint("s1", "i1") &&
        adoptionFingerprint("s1", "i1") !== adoptionFingerprint("s2", "i1"),
      "a changed identity or session makes the publication due again",
    );
    check(
      adoptionFingerprint("s1", null) === "s1",
      "the login session is keyed by its own fingerprint",
    );
  });

  await group("session_adopt names the publisher and omits the path when it has no session", () => {
    const adopt = parsed(
      sessionAdoptRequest(
        DIRECTORY,
        "/group/msime/settings/state/account-session.json",
        "account",
        "settings",
      ),
    );
    const action = adopt.action as Json;
    check(
      adopt.directory === DIRECTORY && !("client_agent" in adopt),
      "directory mode, no agent needed",
    );
    check(
      action.operation === "session_adopt" &&
        action.item === "account" &&
        action.from === "settings",
      "item and publisher",
    );
    check(action.path === "/group/msime/settings/state/account-session.json", "the published file");
    const absent = parsed(sessionAdoptRequest(DIRECTORY, null, "anonymous", "keyboard"))
      .action as Json;
    check(
      !("path" in absent),
      "an absent publication is no path, which Rust reads as the publisher having none",
    );
  });

  await group("adoption results", () => {
    check(
      adoptionResult('{"ok":true,"value":{"outcome":"adopted"}}').outcome === "adopted",
      "adopted",
    );
    check(
      adoptionResult('{"ok":true,"value":{"outcome":"cleared"}}').outcome === "cleared",
      "cleared",
    );
    const refused = adoptionResult('{"ok":false,"error":"account_storage"}');
    check(refused.outcome === "" && refused.error === "account_storage", "refusals keep the code");
    check(
      adoptionResult('{"ok":true,"value":{"outcome":"maybe"}}').error === "account_unavailable",
      "an unknown outcome is a failure",
    );
    check(adoptionResult("garbage").error === "account_unavailable", "garbage is a failure");
  });

  await group(
    "a publication is adopted again only when it changed or the last attempt failed",
    () => {
      const tracker = new AdoptionTracker();
      check(tracker.due("settings", "account", "a"), "a new publication is due");
      tracker.succeeded("settings", "account", "a");
      check(!tracker.due("settings", "account", "a"), "an adopted one is not taken again");
      check(tracker.due("settings", "anonymous", "a"), "items are tracked separately");
      check(tracker.due("keyboard", "account", "a"), "publishers are tracked separately");
      check(tracker.due("settings", "account", "b"), "a changed one is due");
      const first = tracker.failed("settings", "account", "b", "account_storage");
      check(
        first !== null && first.includes("account_storage") && first.includes("M1"),
        "the ownership failure names the rule to check on the device",
      );
      check(
        tracker.failed("settings", "account", "b", "account_storage") === null,
        "the same failure is reported once",
      );
      check(tracker.due("settings", "account", "b"), "a failed publication is retried");
      check(
        tracker.failed("settings", "account", "b", "account_unavailable") !== null,
        "a different failure is reported again",
      );
      check(
        !adoptionFailureMessage("account", "keyboard", "account_unavailable").includes("M1"),
        "only the storage refusal points at M1",
      );
    },
  );

  await group(
    "an unchanged answer given only because the local session was unreadable is tried again",
    () => {
      const unreadable = adoptionResult(
        '{"ok":true,"value":{"outcome":"unchanged","local_unreadable":true}}',
      );
      check(
        unreadable.outcome === "unchanged" && unreadable.localUnreadable,
        "the unreadable local side is read from the reply",
      );
      check(
        !adoptionResult('{"ok":true,"value":{"outcome":"unchanged"}}').localUnreadable,
        "a plain unchanged is not marked",
      );
      const tracker = new AdoptionTracker();
      tracker.answered("keyboard", "account", "k1", unreadable);
      check(
        tracker.due("keyboard", "account", "k1"),
        "the same publication is adopted again once the local session may be readable",
      );
      tracker.answered(
        "keyboard",
        "account",
        "k1",
        adoptionResult('{"ok":true,"value":{"outcome":"unchanged"}}'),
      );
      check(!tracker.due("keyboard", "account", "k1"), "a plain unchanged is remembered");
      tracker.answered(
        "keyboard",
        "account",
        "k2",
        adoptionResult('{"ok":true,"value":{"outcome":"adopted"}}'),
      );
      check(!tracker.due("keyboard", "account", "k2"), "an adopted one is remembered");
    },
  );

  await group("authenticated requests wait for the adoption queued before them", async () => {
    const gate = new SerialGate();
    const order: string[] = [];
    let release: () => void = () => undefined;
    gate.run(
      () =>
        new Promise<void>((resolve) => {
          release = () => {
            order.push("adopted");
            resolve();
          };
        }),
    );
    const request = gate.settled().then(() => {
      order.push("request");
    });
    await new Promise<void>((resolve) => setTimeout(resolve, 5));
    check(order.length === 0, "the request does not start while the adoption is pending");
    release();
    await request;
    check(order.join(",") === "adopted,request", "adoption first, then the request");
  });

  await group("every authenticated request queues an adoption of its own", async () => {
    // 回到前台时网页的请求可能比窗口的 ACTIVE 事件先到：启动时的采用早就做完，前台的那次还没排进来。请求不能只等已经排进来的，否则它拿着键盘已经轮换掉的刷新令牌去刷新。
    const gate = new AdoptionGate();
    const order: string[] = [];
    let published = "start";
    gate.register(async () => {
      order.push(`adopted:${published}`);
    });
    await gate.beforeRequest();
    order.length = 0;
    published = "rotated-by-keyboard";
    await gate.beforeRequest().then(() => {
      order.push("request");
    });
    check(
      order.join(",") === "adopted:rotated-by-keyboard,request",
      `the request adopts what the keyboard published since the last lifecycle event first (got ${order.join(",")})`,
    );
  });

  await group("a request waits for the start adoption still pending", async () => {
    const gate = new AdoptionGate();
    const order: string[] = [];
    let release: () => void = () => undefined;
    let starts = 0;
    gate.register(() => {
      starts += 1;
      if (starts > 1) {
        order.push("adopted-again");
        return Promise.resolve();
      }
      return new Promise<void>((resolve) => {
        release = () => {
          order.push("adopted");
          resolve();
        };
      });
    });
    const request = gate.beforeRequest().then(() => {
      order.push("request");
    });
    await new Promise<void>((resolve) => setTimeout(resolve, 5));
    check(order.length === 0, "the request does not start while the start adoption is pending");
    release();
    await request;
    check(
      order.join(",") === "adopted,adopted-again,request",
      "start adoption, the request's own, then the request",
    );
  });

  await group("before registration a request only waits for what is queued", async () => {
    const gate = new AdoptionGate();
    let ran = 0;
    await gate.run(async () => {
      ran += 1;
    });
    await gate.beforeRequest();
    check(ran === 1, "nothing registered, nothing re-run");
  });

  await group("a failed adoption does not block the queue", async () => {
    const gate = new SerialGate();
    gate.run(async () => {
      throw new Error("adopt crashed");
    });
    let ran = false;
    await gate.run(async () => {
      ran = true;
    });
    check(ran, "the next adoption still runs");
    await gate.settled();
    check(true, "settled resolves after a failure");
  });

  console.log(`${checks - failures} checks passed; ${failures} failed`);
  if (failures !== 0) throw new Error(`${failures} account request checks failed`);
}

// 在计时器里重新抛出，node 以非零退出码结束，与其他测试顶层抛错的效果相同。
main().catch((error: unknown) => {
  setTimeout(() => {
    throw error;
  });
});
