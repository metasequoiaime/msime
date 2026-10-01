import { utf8Length } from "../keyboard/Utf8";

/**
 * Which local settings are account settings, and what happens to them in both directions.
 *
 * The account does not store this host's preference document. It stores a flat map of scalars whose
 * keys the server declares in a schema, and each host maps its own document onto the subset it
 * understands. That indirection is the point: a HarmonyOS phone and an iPhone disagree about almost
 * everything in the document — window chrome, toolbars, hardware chords — but they agree about which
 * input schema the user types in and whether learning is on.
 *
 * Two rules follow from the schema being the server's, and both are load-bearing here.
 *
 * Upload drops any key the schema does not declare. A host that sent one would have its whole write
 * rejected, so a key this host knows about and the server does not simply does not travel yet.
 *
 * Apply reads only keys the schema declares, and refuses outright when a declared key arrives with
 * the wrong type. Silence on an undeclared key is a server that has not caught up; a type mismatch
 * on a declared one is a disagreement about what the field means, and guessing which side is right
 * is how a boolean ends up written into an integer setting.
 *
 * The platform half is namespaced `platform.harmony.*` rather than reusing `platform.android.*`.
 * The two are separate devices with separate keyboards: sharing the namespace would let a HarmonyOS
 * phone overwrite the skin and key spacing of the user's Android keyboard, which is not what a
 * settings sync is for. Until the server declares them, the rule above applies and only the shared
 * `input.*` half travels — which is the half that is genuinely the same product on every host.
 */

export type AccountPreferenceValue = boolean | number | string;
export type AccountPreferenceSettings = Record<string, AccountPreferenceValue>;
export type AccountPreferences = { revision: number; settings: AccountPreferenceSettings };
export type AccountPreferenceField = { type: string };
/**
 * Named rather than written as `AccountPreferenceSchema["fields"]` at the call site: ArkTS rejects
 * indexed access types, and a host that has to name this type has nowhere else to get it.
 */
export type AccountPreferenceFields = Record<string, AccountPreferenceField>;
/** As the page reads it: camelCase, the shape the Tauri hosts hand their settings surface. */
export type AccountPreferenceSchema = {
  fields: AccountPreferenceFields;
  maximumBytes: number;
  updateMode: string;
  revisionRequired: boolean;
};

/** The feedback settings live beside the preference document rather than inside it. */
export type FeedbackValues = {
  soundEnabled: boolean;
  hapticsEnabled: boolean;
  hapticStrength: string;
};

/** Thrown with one of the `account_*` codes the shared page already has a sentence for. */
export class AccountPreferenceError extends Error {
  constructor(code: string) {
    super(code);
    this.name = "AccountPreferenceError";
  }
}

function refuse(code: string): never {
  throw new AccountPreferenceError(code);
}

const MAX_JSON_BYTES = 1024 * 1024;
const MAX_FIELDS = 512;
const MAX_KEY_BYTES = 128;
// A custom keyboard skin can carry roughly 512 KiB of image bytes as base64. The negotiated
// document limit is the real bound, so one string may occupy almost the full 1 MiB envelope.
const MAX_STRING_BYTES = MAX_JSON_BYTES;

const SCHEMES = ["quanpin", "shuangpin", "wubi", "japanese", "korean"];
// Schemes this host types in that the cloud `input.schema` cannot carry (an older device would refuse the whole document), so uploading leaves the field out and the account keeps the scheme it holds.
const LOCAL_ONLY_SCHEMES = ["cantonese", "zhuyin", "vietnamese"];
const SHUANGPIN_PROFILES = ["xiaohe", "ziranma", "shoudao", "microsoft"];
const FREQUENCY_MODES = ["disabled", "pin", "halve", "linear", "promote"];
const LAYOUTS = ["twenty_six_key", "nine_key", "handwriting"];
// The seven global theme ids the shared layer accepts; any other id, a retired skin id included, is refused rather than mapped.
const GLOBAL_THEMES = ["system", "shuishan", "light", "paper", "night", "ink", "custom"];
// What a custom theme may be drawn over: the platform tokens or a built-in theme, never `custom` itself.
const THEME_BASES = ["system", "shuishan", "light", "paper", "night", "ink"];
const THEMES = ["dark", "light", "system"];
const HAPTIC_STRENGTHS = ["light", "medium", "strong"];

function validKey(value: string): boolean {
  if (value.length === 0 || utf8Length(value) > MAX_KEY_BYTES) return false;
  return /^[A-Za-z0-9._-]+$/.test(value);
}

function kindOf(value: AccountPreferenceValue): string {
  if (typeof value === "boolean") return "boolean";
  if (typeof value === "string") return "string";
  return Number.isInteger(value) ? "integer" : "number";
}

/** A declared field accepts its own kind, and a `number` field also accepts a whole number. */
function accepts(declared: string, actual: string): boolean {
  return declared === actual || (declared === "number" && actual === "integer");
}

export function validateAccountPreferences(value: AccountPreferences): void {
  if (!Number.isInteger(value.revision) || value.revision < 0) refuse("account_unavailable");
  const keys = Object.keys(value.settings);
  if (keys.length > MAX_FIELDS) refuse("account_unavailable");
  for (const key of keys) {
    if (!validKey(key)) refuse("account_unavailable");
    const entry = value.settings[key];
    if (typeof entry === "number" && !Number.isFinite(entry)) refuse("account_unavailable");
    if (typeof entry === "string") {
      // eslint-disable-next-line no-control-regex
      if (utf8Length(entry) > MAX_STRING_BYTES || /[\u0000-\u001f\u007f]/.test(entry)) {
        refuse("account_unavailable");
      }
    }
  }
}

export function validatePreferenceSchema(value: AccountPreferenceSchema): void {
  const keys = Object.keys(value.fields);
  if (
    keys.length > MAX_FIELDS ||
    !Number.isInteger(value.maximumBytes) ||
    value.maximumBytes < 1 ||
    value.maximumBytes > MAX_JSON_BYTES ||
    value.updateMode !== "replace" ||
    value.revisionRequired !== true
  ) {
    refuse("account_unavailable");
  }
  for (const key of keys) {
    const field = value.fields[key];
    if (
      !validKey(key) ||
      field === null ||
      typeof field !== "object" ||
      !["boolean", "integer", "number", "string"].includes(field.type)
    ) {
      refuse("account_unavailable");
    }
  }
}

/**
 * Writes this host's values over a snapshot of the cloud document.
 *
 * Fields belonging to other platforms are deliberately kept rather than cleared: the account holds
 * one document for every device the user has, and uploading from a phone must not erase what a
 * desktop put there. Writing a key the schema does not declare, or declaring one type and sending
 * another, is refused rather than dropped — a caller doing that has a bug, and a silent drop turns
 * it into a setting that appears to sync and does not.
 */
export function mergeAccountPreferences(
  base: AccountPreferences,
  replacing: AccountPreferenceSettings,
  schema: AccountPreferenceSchema,
): AccountPreferences {
  validateAccountPreferences(base);
  validatePreferenceSchema(schema);
  const settings: AccountPreferenceSettings = { ...base.settings };
  for (const key of Object.keys(replacing)) {
    const field = schema.fields[key];
    if (field === undefined) refuse("account_invalid");
    if (!accepts(field.type, kindOf(replacing[key]))) refuse("account_invalid");
    settings[key] = replacing[key];
  }
  const merged: AccountPreferences = { revision: base.revision, settings };
  validateAccountPreferences(merged);
  if (utf8Length(JSON.stringify(merged)) > Math.min(schema.maximumBytes, MAX_JSON_BYTES)) {
    refuse("account_invalid");
  }
  return merged;
}

/**
 * The local preference document, read as the loosely typed record the NAPI reply carries.
 *
 * `Object` rather than `unknown` because the HarmonyOS host compiles under the ArkTS subset, which
 * rejects `unknown` outright — including in a cast written at the call site. The helpers below
 * narrow it exactly as they would have.
 */
export type Document = Record<string, Object>;

function record(value: Object | undefined): Document | null {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Document)
    : null;
}

function member(document: Document, key: string): Object | undefined {
  return document[key];
}

function enumerated(value: Object | undefined, allowed: string[], fallback: string): string {
  return typeof value === "string" && allowed.includes(value) ? value : fallback;
}

function flag(value: Object | undefined, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

function whole(value: Object | undefined, fallback: number): number {
  return typeof value === "number" && Number.isInteger(value) ? value : fallback;
}

/**
 * This host's settings, as account scalars.
 *
 * Read from the document rather than from a parsed model, because the host has no model: the shared
 * NAPI hands over the same JSON the settings page edits. A missing member takes the shared default
 * instead of refusing — a document written by an older build is missing the keys that build did not
 * have, and refusing to sync at all because of one absent field helps nobody.
 */
export function localAccountPreferences(
  preferences: Document,
  feedback: FeedbackValues,
): AccountPreferenceSettings {
  const frequency = record(member(preferences, "frequency")) ?? {};
  const customTheme = record(member(preferences, "custom_theme")) ?? {};
  const scheme = member(preferences, "scheme");
  const settings: AccountPreferenceSettings = {
    "input.character_set": flag(member(preferences, "traditional_chinese_output"), false)
      ? "traditional"
      : "simplified",
    "input.shuangpin_schema": enumerated(
      member(preferences, "shuangpin_profile"),
      SHUANGPIN_PROFILES,
      "xiaohe",
    ),
    "input.learning": flag(member(preferences, "learning"), true),
    "input.frequency_mode": enumerated(member(frequency, "mode"), FREQUENCY_MODES, "promote"),
    "input.frequency_trigger_count": whole(member(frequency, "trigger_count"), 1),
    "input.frequency_linear_step": whole(member(frequency, "linear_step"), 1),
    "input.chinese_punctuation": flag(member(preferences, "chinese_punctuation"), true),
    "input.smart_punctuation": flag(member(preferences, "smart_punctuation"), true),
    "input.paired_punctuation": flag(member(preferences, "paired_punctuation"), true),
    "input.wubi_code_hint": flag(member(preferences, "wubi_code_hint"), true),
    "platform.harmony.keyboard_layout": enumerated(
      member(preferences, "touch_keyboard_layout"),
      LAYOUTS,
      "twenty_six_key",
    ),
    "platform.harmony.global_theme": enumerated(
      member(preferences, "global_theme"),
      GLOBAL_THEMES,
      "system",
    ),
    "platform.harmony.custom_theme_base": enumerated(
      member(customTheme, "base"),
      THEME_BASES,
      "system",
    ),
    // The custom theme's keyboard design as JSON; an empty string is "no design" (the base theme's keyboard is drawn), so that state syncs as well.
    "platform.harmony.custom_keyboard_skin": (() => {
      const design = record(member(customTheme, "keyboard"));
      return design === null ? "" : JSON.stringify(design);
    })(),
    "platform.harmony.theme": enumerated(member(preferences, "theme"), THEMES, "system"),
    // The custom theme's external candidate package; an empty string is "none", so clearing it syncs as well.
    "platform.harmony.custom_candidate_skin": (() => {
      const value = member(customTheme, "candidate_skin");
      return typeof value === "string" && externalSkinId(value) ? value : "";
    })(),
    "platform.harmony.touch_key_spacing_tenths": whole(
      member(preferences, "touch_key_spacing_tenths"),
      60,
    ),
    "platform.harmony.touch_row_spacing_tenths": whole(
      member(preferences, "touch_row_spacing_tenths"),
      70,
    ),
    "platform.harmony.keyboard_height_adjustment": whole(
      member(preferences, "touch_keyboard_height_adjustment"),
      0,
    ),
    "platform.harmony.voice_shortcut": flag(member(preferences, "touch_voice_shortcut"), true),
    "platform.harmony.sound_enabled": feedback.soundEnabled,
    "platform.harmony.haptics_enabled": feedback.hapticsEnabled,
    "platform.harmony.haptic_strength": enumerated(
      feedback.hapticStrength,
      HAPTIC_STRENGTHS,
      "medium",
    ),
  };
  if (!(typeof scheme === "string" && LOCAL_ONLY_SCHEMES.includes(scheme))) {
    settings["input.schema"] = enumerated(scheme, SCHEMES, "quanpin");
  }
  return settings;
}

/** Everything the host can write locally from one cloud document. */
export type AppliedPreferences = { preferences: Document; feedback: FeedbackValues | null };

class Reader {
  private readonly values: AccountPreferenceSettings;
  private readonly schema: AccountPreferenceSchema;

  constructor(values: AccountPreferenceSettings, schema: AccountPreferenceSchema) {
    this.values = values;
    this.schema = schema;
  }

  /** Whether the schema declares this key with a compatible type; a clash is a refusal. */
  private declared(key: string, expected: string): boolean {
    const field = this.schema.fields[key];
    if (field === undefined) return false;
    if (accepts(field.type, expected) || accepts(expected, field.type)) return true;
    refuse("account_invalid");
  }

  text(key: string): string | null {
    const value = this.values[key];
    if (typeof value !== "string") return null;
    return this.declared(key, "string") ? value : null;
  }

  boolean(key: string): boolean | null {
    const value = this.values[key];
    if (typeof value !== "boolean") return null;
    return this.declared(key, "boolean") ? value : null;
  }

  integer(key: string): number | null {
    const value = this.values[key];
    if (typeof value !== "number") return null;
    if (!Number.isInteger(value)) refuse("account_invalid");
    return this.declared(key, "integer") ? value : null;
  }

  /** Whether any of these keys is both declared and present, so a store is worth loading. */
  touches(keys: string[]): boolean {
    return keys.some((key) => this.schema.fields[key] !== undefined && key in this.values);
  }
}

/** `catalog::is_external_id`: a safe package folder name that is not a global theme id. */
function externalSkinId(value: string): boolean {
  return (
    value.length > 0 &&
    value.length <= 64 &&
    /^[a-z0-9][a-z0-9._-]*$/.test(value) &&
    !GLOBAL_THEMES.includes(value)
  );
}

function choose(value: string, allowed: string[]): string {
  if (!allowed.includes(value)) refuse("account_invalid");
  return value;
}

function bounded(value: number, minimum: number, maximum: number): number {
  if (value < minimum || value > maximum) refuse("account_invalid");
  return value;
}

/**
 * The cloud document, written onto a copy of the local one.
 *
 * Returns the document to save rather than saving it, so the caller decides the revision it writes
 * against and a refusal part-way through leaves nothing half-applied. The feedback settings come
 * back separately because they live in their own file; `null` means the cloud said nothing about
 * them and the local ones must be left alone rather than rewritten with defaults.
 */
export function applyAccountPreferences(
  local: Document,
  cloud: AccountPreferences,
  schema: AccountPreferenceSchema,
  feedback: FeedbackValues,
): AppliedPreferences {
  validateAccountPreferences(cloud);
  validatePreferenceSchema(schema);
  for (const key of Object.keys(cloud.settings)) {
    const field = schema.fields[key];
    if (field !== undefined && !accepts(field.type, kindOf(cloud.settings[key]))) {
      refuse("account_invalid");
    }
  }
  const reader = new Reader(cloud.settings, schema);
  const preferences: Document = { ...local };

  const scheme = reader.text("input.schema");
  // A scheme this host does not offer (a newer device's Cantonese, Zhuyin or Vietnamese) keeps the local one rather than refusing the whole sync, so the rest of the document still applies.
  if (scheme !== null && SCHEMES.includes(scheme)) preferences.scheme = scheme;
  const characterSet = reader.text("input.character_set");
  if (characterSet !== null) {
    preferences.traditional_chinese_output =
      choose(characterSet, ["simplified", "traditional"]) === "traditional";
  }
  const shuangpin = reader.text("input.shuangpin_schema");
  if (shuangpin !== null) preferences.shuangpin_profile = choose(shuangpin, SHUANGPIN_PROFILES);
  const learning = reader.boolean("input.learning");
  if (learning !== null) preferences.learning = learning;

  const frequency: Document = { ...(record(member(local, "frequency")) ?? {}) };
  let frequencyTouched = false;
  const mode = reader.text("input.frequency_mode");
  if (mode !== null) {
    frequency.mode = choose(mode, FREQUENCY_MODES);
    frequencyTouched = true;
  }
  const triggerCount = reader.integer("input.frequency_trigger_count");
  if (triggerCount !== null) {
    frequency.trigger_count = bounded(triggerCount, 0, 255);
    frequencyTouched = true;
  }
  const linearStep = reader.integer("input.frequency_linear_step");
  if (linearStep !== null) {
    frequency.linear_step = bounded(linearStep, 0, 255);
    frequencyTouched = true;
  }
  if (frequencyTouched) preferences.frequency = frequency;

  const chinesePunctuation = reader.boolean("input.chinese_punctuation");
  if (chinesePunctuation !== null) preferences.chinese_punctuation = chinesePunctuation;
  const smartPunctuation = reader.boolean("input.smart_punctuation");
  if (smartPunctuation !== null) preferences.smart_punctuation = smartPunctuation;
  const pairedPunctuation = reader.boolean("input.paired_punctuation");
  if (pairedPunctuation !== null) preferences.paired_punctuation = pairedPunctuation;
  const wubiCodeHint = reader.boolean("input.wubi_code_hint");
  if (wubiCodeHint !== null) preferences.wubi_code_hint = wubiCodeHint;

  const layout = reader.text("platform.harmony.keyboard_layout");
  if (layout !== null) preferences.touch_keyboard_layout = choose(layout, LAYOUTS);
  const globalTheme = reader.text("platform.harmony.global_theme");
  if (globalTheme !== null) preferences.global_theme = choose(globalTheme, GLOBAL_THEMES);
  // The custom theme is one nested record; its synced parts are written onto a copy so the ones the account does not carry (the candidate colour pickers) stay as they were.
  const customTheme: Document = { ...(record(member(local, "custom_theme")) ?? {}) };
  let customThemeTouched = false;
  const themeBase = reader.text("platform.harmony.custom_theme_base");
  if (themeBase !== null) {
    // `system` is the default and the shared document omits it, so it is written by removing the member.
    if (choose(themeBase, THEME_BASES) === "system") {
      delete customTheme.base;
    } else {
      customTheme.base = themeBase;
    }
    customThemeTouched = true;
  }
  const customSkin = reader.text("platform.harmony.custom_keyboard_skin");
  if (customSkin !== null) {
    if (customSkin.length === 0) {
      delete customTheme.keyboard;
    } else {
      let design: Object | null;
      try {
        design = JSON.parse(customSkin) as Object;
      } catch {
        refuse("account_invalid");
      }
      if (design === null || record(design) === null) refuse("account_invalid");
      customTheme.keyboard = design;
    }
    customThemeTouched = true;
  }
  const candidateSkin = reader.text("platform.harmony.custom_candidate_skin");
  if (candidateSkin !== null) {
    if (candidateSkin.length === 0) {
      delete customTheme.candidate_skin;
    } else {
      if (!externalSkinId(candidateSkin)) refuse("account_invalid");
      customTheme.candidate_skin = candidateSkin;
    }
    customThemeTouched = true;
  }
  if (customThemeTouched) preferences.custom_theme = customTheme;
  const theme = reader.text("platform.harmony.theme");
  if (theme !== null) preferences.theme = choose(theme, THEMES);
  const keySpacing = reader.integer("platform.harmony.touch_key_spacing_tenths");
  if (keySpacing !== null) preferences.touch_key_spacing_tenths = bounded(keySpacing, 0, 255);
  const rowSpacing = reader.integer("platform.harmony.touch_row_spacing_tenths");
  if (rowSpacing !== null) preferences.touch_row_spacing_tenths = bounded(rowSpacing, 0, 255);
  const heightAdjustment = reader.integer("platform.harmony.keyboard_height_adjustment");
  if (heightAdjustment !== null) {
    preferences.touch_keyboard_height_adjustment = bounded(heightAdjustment, -128, 127);
  }
  const voiceShortcut = reader.boolean("platform.harmony.voice_shortcut");
  if (voiceShortcut !== null) preferences.touch_voice_shortcut = voiceShortcut;

  const feedbackKeys = [
    "platform.harmony.sound_enabled",
    "platform.harmony.haptics_enabled",
    "platform.harmony.haptic_strength",
  ];
  if (!reader.touches(feedbackKeys)) return { preferences, feedback: null };
  const next: FeedbackValues = { ...feedback };
  const soundEnabled = reader.boolean("platform.harmony.sound_enabled");
  if (soundEnabled !== null) next.soundEnabled = soundEnabled;
  const hapticsEnabled = reader.boolean("platform.harmony.haptics_enabled");
  if (hapticsEnabled !== null) next.hapticsEnabled = hapticsEnabled;
  const hapticStrength = reader.text("platform.harmony.haptic_strength");
  if (hapticStrength !== null) next.hapticStrength = choose(hapticStrength, HAPTIC_STRENGTHS);
  return { preferences, feedback: next };
}
