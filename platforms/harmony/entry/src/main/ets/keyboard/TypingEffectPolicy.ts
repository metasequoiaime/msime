/**
 * What the 2in1 typing effect draws and when the combo tier-up sound plays, decided without a device.
 *
 * The combo itself is counted by host-api (`msime_client_typing_effect`, `crates/host-api/src/key_sound/effect.rs`); this host only decodes the integer it answers with and turns it into a flash of the candidate card and a combo badge. The desktop player queues the tier-up sound itself, but host-api links no audio stack on HarmonyOS, so this host plays it: the key pack's commit sample raised `TIER_SEMITONES` per tier, as `player.rs` pitches it.
 */
import { PluginPreferenceDocument } from "./KeySoundPolicy";

/** Event codes, in the low byte of the event `msime_client_typing_effect` takes. 0-3 are the key sound classes. */
export const TYPING_EFFECT_COMMIT: number = 4;
/** A delete that did not go through a key sound; it ends the combo as Backspace does. */
export const TYPING_EFFECT_BACKSPACE: number = 5;
/** Flag: the key is an auto-repeat of a held key; drawn, not counted. */
export const TYPING_EFFECT_AUTO_REPEAT: number = 0x100;
/** Flag: sounds must stay quiet now; the tier-up sound is not reported due. */
export const TYPING_EFFECT_MUTED: number = 0x200;

/** `COMBO_MILESTONES` in client-core: the counts that move the combo up a tier. */
export const COMBO_MILESTONES: number[] = [10, 25, 50, 100];
/** `COMBO_IDLE_RESET_MILLIS` in client-core: the combo ends after this long without a counted key, so the badge goes with it. */
export const COMBO_IDLE_RESET_MILLIS: number = 3000;
/** `TIER_SEMITONES` in host-api: the tier-up sound rises this many semitones per tier, so the fourth tier is an octave up. */
export const TIER_SEMITONES: number = 3;
/** How long one flash of the candidate card takes to fade, in milliseconds. */
export const FLASH_MILLIS: number = 160;
/** The badge appears from this many keys on, so a single key is not called a combo. */
const BADGE_MINIMUM: number = 2;

const ANSWER_COUNT: number = 0xffff;
const ANSWER_TIER_UP: number = 1 << 16;
const ANSWER_STYLE_SHIFT: number = 17;
const ANSWER_STYLE_MASK: number = 0x7;
const ANSWER_TIER_SOUND: number = 1 << 20;

const DEFAULT_INTENSITY: number = 50;

/** `EffectStyle::code`, as the answer carries it in bits 17-19. */
export enum TypingEffectStyle {
  OFF = 0,
  FLASH = 1,
  SPARKS = 2,
  POWER_MODE = 3,
}

/** The typing effect settings of one preference document. */
export interface TypingEffectSettings {
  readonly style: TypingEffectStyle;
  /** 0-100: how strong the flash and the badge pulse are. */
  readonly intensity: number;
  readonly comboCounter: boolean;
  /** The tier-up sound: on only with the combo counter, as host-api's `tier_sound` holds. */
  readonly tierSound: boolean;
  /** The selected effect pack's id, "" for none. Its style comes back in each answer; `resolve` fills in the rest. */
  readonly pack: string;
  /** How long one flash takes to fade: `FLASH_MILLIS` unless the pack sets `duration_ms`. */
  readonly flashMillis: number;
  /** The flash colour as `#RRGGBB`, the pack's first colour; `undefined` uses the candidate accent. */
  readonly color: string | undefined;
}

/** The value of `msime_client_typing_effect_settings`, as host-api resolves it. Only the fields this host draws are read; HarmonyOS draws no sparks, so `particles` is ignored. */
export interface ResolvedTypingEffectDocument {
  pack: string | null;
  issue: string | null;
  intensity: number;
  colors: string[];
  duration_ms: number | null;
}

/** One answer of `msime_client_typing_effect`, unpacked. */
export interface TypingEffect {
  /** The combo count; 0 while the combo counter is off. */
  readonly count: number;
  /** This key moved the combo up a tier. */
  readonly tierUp: boolean;
  readonly style: TypingEffectStyle;
  /** The tier-up sound is due, and this host plays it. */
  readonly tierSound: boolean;
}

export const TYPING_EFFECTS_OFF: TypingEffectSettings = {
  style: TypingEffectStyle.OFF,
  intensity: DEFAULT_INTENSITY,
  comboCounter: false,
  tierSound: false,
  pack: "",
  flashMillis: FLASH_MILLIS,
  color: undefined,
};

const COLOR_PATTERN: RegExp = /^#[0-9a-fA-F]{6}$/;
/** `effect.duration_ms`'s bounds in client-core. */
const MIN_FLASH_MILLIS: number = 60;
const MAX_FLASH_MILLIS: number = 1500;

export class TypingEffectPolicy {
  /** The settings `preferences.plugins` asks for; a missing record or an unknown style is off. */
  static settings(plugins: PluginPreferenceDocument | undefined): TypingEffectSettings {
    if (plugins === undefined) {
      return TYPING_EFFECTS_OFF;
    }
    const intensity: number =
      typeof plugins.effect_intensity === "number" &&
      plugins.effect_intensity >= 0 &&
      plugins.effect_intensity <= 100
        ? Math.round(plugins.effect_intensity)
        : DEFAULT_INTENSITY;
    const comboCounter: boolean = plugins.combo_counter === true;
    return {
      style: TypingEffectPolicy.style(plugins.effect_style),
      intensity: intensity,
      comboCounter: comboCounter,
      tierSound: comboCounter && plugins.combo_tier_sound === true,
      pack: typeof plugins.effect_pack === "string" ? plugins.effect_pack : "",
      flashMillis: FLASH_MILLIS,
      color: undefined,
    };
  }

  /** `settings` with what host-api resolved for the session: a selected pack's intensity, flash length and colour replace the preference values. Without a pack, or when the pack did not load (host-api then answers style off), the preference values stand. */
  static resolve(
    settings: TypingEffectSettings,
    resolved: ResolvedTypingEffectDocument,
  ): TypingEffectSettings {
    if (settings.pack === "" || resolved.pack === null || resolved.issue !== null) {
      return settings;
    }
    const intensity: number =
      typeof resolved.intensity === "number" && resolved.intensity >= 0 && resolved.intensity <= 100
        ? Math.round(resolved.intensity)
        : settings.intensity;
    const flashMillis: number =
      typeof resolved.duration_ms === "number" && Number.isFinite(resolved.duration_ms)
        ? Math.min(Math.max(Math.round(resolved.duration_ms), MIN_FLASH_MILLIS), MAX_FLASH_MILLIS)
        : FLASH_MILLIS;
    const first: string | undefined = Array.isArray(resolved.colors)
      ? resolved.colors[0]
      : undefined;
    return {
      style: settings.style,
      intensity: intensity,
      comboCounter: settings.comboCounter,
      tierSound: settings.tierSound,
      pack: settings.pack,
      flashMillis: flashMillis,
      color: typeof first === "string" && COLOR_PATTERN.test(first) ? first : undefined,
    };
  }

  /** The style a preference names; anything else is off. */
  static style(name: string | undefined): TypingEffectStyle {
    if (name === "flash") {
      return TypingEffectStyle.FLASH;
    }
    if (name === "sparks") {
      return TypingEffectStyle.SPARKS;
    }
    if (name === "power_mode") {
      return TypingEffectStyle.POWER_MODE;
    }
    return TypingEffectStyle.OFF;
  }

  /** Whether keys are handed to `msime_client_typing_effect` at all: the header asks for the call only while a style is drawn, an effect pack is selected or the combo is counted. */
  static active(settings: TypingEffectSettings): boolean {
    return (
      settings.style !== TypingEffectStyle.OFF || settings.pack !== "" || settings.comboCounter
    );
  }

  /** Unpack one answer. */
  static decode(answer: number): TypingEffect {
    const bits: number = answer >>> 0;
    return {
      count: bits & ANSWER_COUNT,
      tierUp: (bits & ANSWER_TIER_UP) !== 0,
      style: ((bits >>> ANSWER_STYLE_SHIFT) & ANSWER_STYLE_MASK) as TypingEffectStyle,
      tierSound: (bits & ANSWER_TIER_SOUND) !== 0,
    };
  }

  /** How many milestones `count` has reached: the tier host-api's combo is at. */
  static tier(count: number): number {
    let tier: number = 0;
    for (const milestone of COMBO_MILESTONES) {
      if (count >= milestone) {
        tier += 1;
      }
    }
    return tier;
  }

  /** The semitone the tier-up sound of `tier` plays at. */
  static tierSemitone(tier: number): number {
    return tier * TIER_SEMITONES;
  }

  /** Every pitch a tier-up sound may play at, for preparing the commit sample ahead of the key path. */
  static tierSemitones(): number[] {
    return COMBO_MILESTONES.map((_milestone: number, index: number): number =>
      TypingEffectPolicy.tierSemitone(index + 1),
    );
  }

  /** The candidate card's flash opacity at its peak, 0 for no flash. Stronger styles flash brighter, a tier-up brighter still, and the intensity scales it; it stays translucent so the candidates are always readable. */
  static flashOpacity(effect: TypingEffect, intensity: number): number {
    let base: number = 0;
    if (effect.style === TypingEffectStyle.FLASH) {
      base = 0.16;
    } else if (effect.style === TypingEffectStyle.SPARKS) {
      base = 0.24;
    } else if (effect.style === TypingEffectStyle.POWER_MODE) {
      base = 0.32;
    }
    if (effect.tierUp) {
      base *= 1.5;
    }
    const scaled: number = (base * Math.min(Math.max(intensity, 0), 100)) / DEFAULT_INTENSITY;
    return Math.min(scaled, 0.6);
  }

  /** The combo badge's text, or an empty string when there is none to show. */
  static badge(count: number): string {
    return count >= BADGE_MINIMUM ? `连击 ×${count}` : "";
  }

  /** How far the badge swells on a counted key before settling back: only power mode pulses it, harder on a tier-up. */
  static badgeScale(effect: TypingEffect, intensity: number): number {
    if (effect.style !== TypingEffectStyle.POWER_MODE || effect.count < BADGE_MINIMUM) {
      return 1;
    }
    const strength: number = Math.min(Math.max(intensity, 0), 100) / 100;
    return 1 + (effect.tierUp ? 0.5 : 0.2) * strength;
  }
}
