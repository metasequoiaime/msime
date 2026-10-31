import {
  ConsentRecord,
  PRIVACY_CONSENT_ENTRY,
  PRIVACY_CONSENT_FILE,
  PRIVACY_CONSENT_REQUIRED,
  PRIVACY_POLICY_URL,
  PRIVACY_POLICY_VERSION,
  PrivacyConsentPolicy,
  USER_AGREEMENT_URL,
} from "../entry/src/main/ets/privacy/PrivacyConsentPolicy";
import {
  KEYBOARD_ENTRIES,
  SETTINGS_ENTRIES,
  SharedStateMirrorPolicy,
} from "../entry/src/main/ets/keyboard/SharedStateMirrorPolicy";
import { TextPolicy } from "../entry/src/main/ets/keyboard/TextPolicy";

let failures = 0;
let checks = 0;

/** 放在本地定义，这样测试套件不需要 node 类型定义，本仓库也没有这些定义。 */
function group(name: string, body: () => void): void {
  try {
    body();
    console.log(`  ok  ${name}`);
  } catch (error) {
    failures++;
    console.log(`FAIL  ${name}`);
    console.log(`      ${error instanceof Error ? error.message : String(error)}`);
  }
}

function check(condition: boolean, message: string): void {
  checks++;
  if (!condition) {
    throw new Error(message);
  }
}

const V = PRIVACY_POLICY_VERSION;

/** 存盘再读回来，和设置应用写、键盘读走的是同一条路。 */
function roundTrip(record: ConsentRecord): ConsentRecord | null {
  return PrivacyConsentPolicy.parse(PrivacyConsentPolicy.serialize(record));
}

console.log("PrivacyConsentPolicy");

group("a first launch prompts and keeps the network closed", () => {
  check(PrivacyConsentPolicy.shouldPrompt(null, V), "no record means the dialog is shown");
  check(!PrivacyConsentPolicy.networkAllowed(null, V), "no record means no network");
  const state = PrivacyConsentPolicy.state(null, V);
  check(!state.granted && !state.decided, "the page is told nothing was decided");
  // 老用户升级后同样没有记录，所以也会看到一次弹窗。
  check(PrivacyConsentPolicy.parse(null) === null, "a missing file reads as no record");
  check(PrivacyConsentPolicy.parse("") === null, "an empty file reads as no record");
  check(PrivacyConsentPolicy.parse("{not json") === null, "a damaged file reads as no record");
  check(
    PrivacyConsentPolicy.parse('{"format":1,"version":1,"decision":"maybe","decided_at_ms":0}') ===
      null,
    "an unknown decision reads as no record",
  );
  check(
    PrivacyConsentPolicy.parse(
      '{"format":2,"version":1,"decision":"granted","decided_at_ms":0}',
    ) === null,
    "a record format this build does not know reads as no record",
  );
  check(
    PrivacyConsentPolicy.parse(
      '{"format":1,"version":0,"decision":"granted","decided_at_ms":0}',
    ) === null,
    "a version below 1 reads as no record",
  );
});

group("agreeing persists the version and stops the prompt", () => {
  const record = roundTrip(PrivacyConsentPolicy.record(true, V, 1700000000000));
  check(record !== null, "the agreed record reads back");
  check(
    record!.decision === "granted" && record!.version === V,
    "the decision and version are kept",
  );
  check(record!.decided_at_ms === 1700000000000, "the decision time is kept");
  check(!PrivacyConsentPolicy.shouldPrompt(record, V), "an agreed user is not asked again");
  check(PrivacyConsentPolicy.networkAllowed(record, V), "an agreed user may use the network");
  const state = PrivacyConsentPolicy.state(record, V);
  check(state.granted && state.decided, "the page is told consent was given");
});

group("refusing keeps the network closed without asking again on every start", () => {
  const record = roundTrip(PrivacyConsentPolicy.record(false, V, 1));
  check(record !== null && record.decision === "refused", "the refusal reads back");
  check(!PrivacyConsentPolicy.networkAllowed(record, V), "a refusal means no network");
  check(
    !PrivacyConsentPolicy.shouldPrompt(record, V),
    "a refusal is not met by the dialog at every start",
  );
  check(
    !PrivacyConsentPolicy.shouldPrompt(record, V + 1),
    "nor after a policy update: nothing was agreed to re-confirm",
  );
  const state = PrivacyConsentPolicy.state(record, V);
  check(!state.granted && state.decided, "the page is told the user decided against it");
});

group("a policy version bump asks again and closes the network until then", () => {
  const old = roundTrip(PrivacyConsentPolicy.record(true, V, 1));
  check(PrivacyConsentPolicy.shouldPrompt(old, V + 1), "consent to an older policy prompts again");
  check(!PrivacyConsentPolicy.networkAllowed(old, V + 1), "and no longer opens the network");
  const renewed = roundTrip(PrivacyConsentPolicy.record(true, V + 1, 2));
  check(
    !PrivacyConsentPolicy.shouldPrompt(renewed, V + 1),
    "agreeing to the new version settles it",
  );
  check(PrivacyConsentPolicy.networkAllowed(renewed, V + 1), "and opens the network again");
  check(
    PrivacyConsentPolicy.networkAllowed(renewed, V),
    "a newer consent also covers an older build",
  );
});

group("refusal blocks every networked bridge request", () => {
  const networked: [string, string][] = [
    ["account", '{"operation":"status"}'],
    ["account", '{"operation":"send_code","channel":"email"}'],
    ["account", '{"operation":"google_sign_in"}'],
    ["account", '{"operation":"feedback","text":"synthetic"}'],
    ["chat", '{"operation":"chat"}'],
    ["cloud_dictionary", '{"operation":"list"}'],
    ["cloud_dictionary_snapshot", '{"operation":"upload"}'],
    ["cloud_clipboard", '{"operation":"list"}'],
    ["cloud_clipboard", '{"operation":"devices"}'],
    [
      "cloud_clipboard_webdav_probe",
      '{"url":"https://dav.example.test/dav","username":"synthetic","password":"synthetic"}',
    ],
    ["dictionary_sync", '{"operation":"run","trigger":"opened"}'],
    // 改词之后的那一次在发送的防抖到期时也会联网。
    ["dictionary_sync", '{"operation":"run","trigger":"local_change"}'],
    ["dictionary_sync", '{"operation":"devices"}'],
    ["dictionary_sync", '{"operation":"approve","device_id":"0123456789abcdef0123456789abcdef"}'],
    [
      "dictionary_sync",
      '{"operation":"probe","url":"https://dav.example.test/dav","username":"synthetic","password":"synthetic"}',
    ],
    ["dictionary_sync", "not json"],
    ["settings_sync", '{"operation":"preferences_upload"}'],
    ["ai_skin", '{"operation":"generate"}'],
    ["community_resource", '{"operation":"list"}'],
    ["community_skin", '{"operation":"keyboard_skin_install"}'],
    ["community_plugin", '{"operation":"plugin_list"}'],
    ["community_block", '{"operation":"block","kind":"skins","id":"synthetic"}'],
    ["community_block", '{"operation":"block_list"}'],
    ["community_block", '{"operation":"unblock","id":"synthetic"}'],
    ["community_block", "not json"],
    ["typing_upload", '{"operation":"sync"}'],
    ["ai_models", "{}"],
    ["ai_test", "{}"],
    ["api_credential", '{"service":"tencent"}'],
    ["update_check", '{"current_version":"1.0.0"}'],
    ["voice_local_model", '{"operation":"install","id":"synthetic"}'],
    ["account", '{"operation":"huawei_sign_in"}'],
    ["account", '{"operation":"huawei_challenge"}'],
    ["account", '{"operation":"huawei_login","id_token":"synthetic"}'],
    ["account", "not json"],
  ];
  for (const [kind, payload] of networked) {
    check(
      PrivacyConsentPolicy.refuses(kind, payload, false),
      `${kind} ${payload} is refused without consent`,
    );
    check(
      !PrivacyConsentPolicy.refuses(kind, payload, true),
      `${kind} ${payload} goes through with consent`,
    );
  }
  const refusal = JSON.parse(PrivacyConsentPolicy.refusal()) as { ok: boolean; error: string };
  check(
    refusal.ok === false && refusal.error === PRIVACY_CONSENT_REQUIRED,
    "the refusal is the C ABI failure shape",
  );
});

group("local work keeps going without consent", () => {
  const local: [string, string][] = [
    ["account", '{"operation":"google_cancel"}'],
    ["ai_skin", '{"operation":"cancel","request_id":"synthetic"}'],
    ["community_resource", '{"operation":"store_reply","item":{}}'],
    ["community_resource", '{"operation":"remove_reply","id":"synthetic"}'],
    ["community_skin", '{"operation":"finish_trial","id":"synthetic","keep":true}'],
    ["voice_local_model", '{"operation":"list"}'],
    ["voice_local_model", '{"operation":"import","id":"synthetic"}'],
    ["voice_local_model", '{"operation":"remove","id":"synthetic"}'],
    ["voice_local_model", '{"operation":"cancel","id":"synthetic"}'],
    ["custom_skin_library", ""],
    ["vocabulary_review", '{"operation":"status"}'],
    ["repair_preferences", ""],
    ["dictionary_collections", '{"operation":"load"}'],
    ["dictionary_sync", '{"operation":"status"}'],
    ["dictionary_scel_text", "{}"],
    ["statistics_auth_settings", "{}"],
    ["statistics_auth", ""],
    ["typing_statistics", '{"operation":"summary"}'],
    ["helpcode_import", ""],
    ["plugin_import", "{}"],
    ["candidate_skin_editor", '{"operation":"check"}'],
    ["save_export", "{}"],
  ];
  for (const [kind, payload] of local) {
    check(
      !PrivacyConsentPolicy.refuses(kind, payload, false),
      `${kind} ${payload} is local and allowed`,
    );
  }
});

group("dictionary updates download only on automatic and check", () => {
  check(
    PrivacyConsentPolicy.dictionaryActionNeedsConsent(
      '{"operation":"updates","command":"automatic"}',
    ),
    "the daily automatic check needs consent",
  );
  check(
    PrivacyConsentPolicy.dictionaryActionNeedsConsent('{"operation":"updates","command":"check"}'),
    "a manual check needs consent",
  );
  for (const command of ["status", "configure", "rollback"]) {
    check(
      !PrivacyConsentPolicy.dictionaryActionNeedsConsent(
        `{"operation":"updates","command":"${command}"}`,
      ),
      `${command} stays local`,
    );
  }
  check(
    !PrivacyConsentPolicy.dictionaryActionNeedsConsent('{"operation":"list"}'),
    "dictionary edits stay local",
  );
  check(
    !PrivacyConsentPolicy.dictionaryActionNeedsConsent("not json"),
    "an unreadable action is left to the store",
  );
});

group("the dialog offers both choices and links the real privacy policy", () => {
  const copy = PrivacyConsentPolicy.COPY;
  check(copy.agree === "同意" && copy.refuse === "不同意", "the buttons read 同意 and 不同意");
  check(
    ![copy.intro, ...copy.points, copy.refusalNote].join("").includes("AI"),
    "the dialog does not introduce AI features while AiGenerationGate hides them on HarmonyOS",
  );
  check(
    PRIVACY_POLICY_URL === "https://msime.app/privacy/",
    "the policy link is the published page",
  );
  check(
    USER_AGREEMENT_URL === "https://msime.app/terms/",
    "the agreement link is the published page",
  );
  check(
    PrivacyConsentPolicy.DOCUMENTS.length === 2 &&
      PrivacyConsentPolicy.DOCUMENTS[0].title === "《用户协议》" &&
      PrivacyConsentPolicy.DOCUMENTS[0].url === USER_AGREEMENT_URL &&
      PrivacyConsentPolicy.DOCUMENTS[1].title === "《隐私政策》" &&
      PrivacyConsentPolicy.DOCUMENTS[1].url === PRIVACY_POLICY_URL,
    "the dialog links the user agreement and the privacy policy, both live pages",
  );
  check(
    copy.refusalNote.includes("本机打字"),
    "the dialog says local typing keeps working after a refusal",
  );
  check(copy.refusalNote.includes("「我的 › 隐私」"), "and where to agree later");
});

group("the keyboard receives the consent through the shared sandbox", () => {
  const entry = SETTINGS_ENTRIES.find((item) => item.name === PRIVACY_CONSENT_ENTRY);
  check(entry !== undefined, "settings publishes the consent record");
  check(entry!.kind === "file" && entry!.base === "state", "as a file under files/state");
  check(entry!.path === PRIVACY_CONSENT_FILE, "at the path the keyboard reads");
  check(!entry!.fullModeOnly, "a keyboard in basic access mode still reads it");
  check(
    !KEYBOARD_ENTRIES.some((item) => item.name === PRIVACY_CONSENT_ENTRY),
    "only the settings app decides consent; the keyboard never publishes it back",
  );
  check(
    SharedStateMirrorPolicy.importDecision(PRIVACY_CONSENT_ENTRY) === "copy",
    "the keyboard copies it as is",
  );
});

group("a link the system cannot open tells the user where to go", () => {
  const terms = TextPolicy.unopenedLinkNotice(USER_AGREEMENT_URL);
  check(terms.includes("msime.app/terms/"), "the notice names the user agreement address");
  check(!terms.includes("https://"), "the address is shown without the scheme");
  check(
    TextPolicy.unopenedLinkNotice(PRIVACY_POLICY_URL).includes("msime.app/privacy/"),
    "the notice names the privacy policy address",
  );
  check(
    TextPolicy.unopenedLinkNotice("http://example.com/a") ===
      "没能打开浏览器，请在浏览器中访问 example.com/a",
    "a plain http link loses its scheme too",
  );
});

console.log(`\n${checks} checks, ${failures} failed`);
if (failures > 0) {
  throw new Error(`${failures} privacy consent group(s) failed`);
}
