/**
 * What the 2in1 key sounds, typing melody, commit and achievement sounds play, decided without a device.
 *
 * The desktop hosts play through host-api's player (`crates/host-api/src/key_sound`), which HarmonyOS does not link; this host plays the same packs through SoundPool instead. Which pack answers which event, how a melody steps and when it starts over are that player's rules (`Selection::of`, `sounds_for`, `Melody::step`), ported here so the two cannot hear different things in the same preferences. Validating a pack is not ported: `msime_client_key_sound_pack` answers with the files client-core's validation accepted.
 */

import { AudioVolumePolicy } from "./AudioVolumePolicy";

/** A key's sound class, numbered as `msime_client_key_sound` numbers them. */
export enum KeySoundClass {
  DEFAULT = 0,
  SPACE = 1,
  ENTER = 2,
  BACKSPACE = 3,
}

/** What happened that may make a sound. */
export enum KeySoundEvent {
  KEY,
  COMMIT,
  ACHIEVEMENT,
}

/** The shared `preferences.plugins.key_sound` record, as far as this host reads it. */
export interface KeySoundPreferenceDocument {
  enabled?: boolean;
  /** "keys" or "melody". */
  mode?: string;
  pack?: string;
  /** 0-100. */
  volume?: number;
}

export interface SwitchPreferenceDocument {
  enabled?: boolean;
}

export interface MelodyPreferenceDocument {
  pack?: string;
}

/** `preferences.plugins.music`, which `MusicPolicy` reads. */
export interface MusicPreferenceDocument {
  enabled?: boolean;
  pack?: string;
  /** 0-100. */
  volume?: number;
}

/** `preferences.plugins`. Absent while every part is at its default, which the shared store leaves out of the document. */
export interface PluginPreferenceDocument {
  key_sound?: KeySoundPreferenceDocument;
  commit_sound?: SwitchPreferenceDocument;
  melody?: MelodyPreferenceDocument;
  achievements?: SwitchPreferenceDocument;
  music?: MusicPreferenceDocument;
  /** "off", "flash", "sparks" or "power_mode"; `TypingEffectPolicy` reads it. */
  effect_style?: string;
  /** 0-100. */
  effect_intensity?: number;
  /** The selected effect pack's id, "" or missing for none; host-api resolves it. */
  effect_pack?: string;
  combo_counter?: boolean;
  combo_tier_sound?: boolean;
}

/** The effect settings of one preference document. Background music is `MusicPolicy`'s. */
export interface KeySoundSettings {
  readonly key: boolean;
  /** Keys play the melody pack's next note instead of their own sample. */
  readonly melody: boolean;
  readonly commit: boolean;
  readonly achievements: boolean;
  readonly pack: string;
  readonly melodyPack: string;
  /** 0-100, for every effect sound. */
  readonly volume: number;
  /** The combo tier-up sound, the key pack's commit sample raised per tier: on only with the combo counter, as host-api's `tier_sound` holds. */
  readonly tierSound: boolean;
}

/** The packs a set of settings plays from; null where no sound uses one. */
export interface KeySoundSelection {
  readonly pack: string | null;
  readonly melodyPack: string | null;
  /** The key pack's commit sample is prepared at the tier-up pitches too. */
  readonly tierSound: boolean;
}

/** What one stat of a pack manifest reports, as far as telling a replaced manifest from the one loaded goes. */
export interface KeySoundManifestStat {
  readonly ino: number | bigint;
  readonly size: number;
  readonly mtime: number;
}

/** `sounds` of `msime_client_key_sound_pack`: absolute paths, null where the pack has none. */
export interface KeySoundPackSounds {
  default: string | null;
  space: string | null;
  enter: string | null;
  backspace: string | null;
  commit: string | null;
  achievement: string | null;
}

export interface KeySoundPackSequence {
  sample: string;
  semitones: number[];
  /** "key" or "commit". */
  advance: string;
}

/** The value `msime_client_key_sound_pack` answers with. */
export interface KeySoundPackFiles {
  id: string;
  name: string;
  license: string;
  builtin: boolean;
  /** "keys" or "sequence". */
  mode: string;
  sounds: KeySoundPackSounds;
  sequence: KeySoundPackSequence | null;
  max_sample_millis: number;
  melody_idle_reset_millis: number;
}

/** One file to prepare, and the pitches it is played at. */
export interface KeySoundSampleRequest {
  readonly file: string;
  readonly semitones: number[];
}

/** One sound to play: a file at a pitch. */
export interface KeySoundCue {
  readonly file: string;
  readonly semitone: number;
}

const DEFAULT_SOUND_PACK: string = "default";
const DEFAULT_MELODY_PACK: string = "twinkle";
const DEFAULT_VOLUME: number = 50;

const KEYCODE_SPACE: number = 2050;
const KEYCODE_ENTER: number = 2054;
const KEYCODE_DEL: number = 2055;
const KEYCODE_FORWARD_DEL: number = 2071;
const KEYCODE_NUMPAD_ENTER: number = 2119;

export const KEY_SOUNDS_OFF: KeySoundSettings = {
  key: false,
  melody: false,
  commit: false,
  achievements: false,
  pack: DEFAULT_SOUND_PACK,
  melodyPack: DEFAULT_MELODY_PACK,
  volume: DEFAULT_VOLUME,
  tierSound: false,
};

/** Steps through a melody note by note, as host-api's `Melody` does. */
export class KeySoundMelody {
  private next: number = 0;
  private last: number = -1;

  /** The semitone of the next note, or null for an empty tune. The tune starts over at its end, and after `idleMillis` without a note, so a pause in typing begins it again. */
  step(semitones: number[], now: number, idleMillis: number): number | null {
    if (semitones.length === 0) {
      return null;
    }
    if (this.next >= semitones.length || (this.last >= 0 && now - this.last >= idleMillis)) {
      this.next = 0;
    }
    this.last = now;
    const note: number = semitones[this.next];
    this.next += 1;
    return note;
  }

  reset(): void {
    this.next = 0;
    this.last = -1;
  }
}

export class KeySoundPolicy {
  /** The settings `preferences.plugins` asks for; a missing record is every default, which is all of them off. */
  static settings(plugins: PluginPreferenceDocument | undefined): KeySoundSettings {
    if (plugins === undefined) {
      return KEY_SOUNDS_OFF;
    }
    const key: KeySoundPreferenceDocument = plugins.key_sound ?? {};
    const volume: number =
      typeof key.volume === "number" && key.volume >= 0 && key.volume <= 100
        ? Math.round(key.volume)
        : DEFAULT_VOLUME;
    return {
      key: key.enabled === true,
      melody: key.mode === "melody",
      commit: plugins.commit_sound?.enabled === true,
      achievements: plugins.achievements?.enabled === true,
      pack: typeof key.pack === "string" ? key.pack : DEFAULT_SOUND_PACK,
      melodyPack:
        typeof plugins.melody?.pack === "string" ? plugins.melody.pack : DEFAULT_MELODY_PACK,
      volume: volume,
      tierSound: plugins.combo_counter === true && plugins.combo_tier_sound === true,
    };
  }

  /** Whether anything plays at all. Nothing is loaded, and no SoundPool created, until this holds. */
  static wanted(settings: KeySoundSettings): boolean {
    return settings.key || settings.commit || settings.achievements || settings.tierSound;
  }

  /** The key pack when a key, commit, achievement or tier-up sound uses it, and the melody pack when keys play the melody. */
  static selection(settings: KeySoundSettings): KeySoundSelection {
    return {
      pack:
        (settings.key && !settings.melody) ||
        settings.commit ||
        settings.achievements ||
        settings.tierSound
          ? settings.pack
          : null,
      melodyPack: settings.key && settings.melody ? settings.melodyPack : null,
      tierSound: settings.tierSound,
    };
  }

  /** SoundPool's volume, 0-1. The setting scales amplitude, as host-api's `decibels` does: 50 is half, -6 dB. */
  static gain(settings: KeySoundSettings): number {
    return AudioVolumePolicy.gain(settings.volume);
  }

  /**
   * Whether key-downs are silent in this state: in a password field, so the rhythm of a password is never heard, and in English mode, as on every desktop host (Windows hears only the keys its input method composes). The English candidate mode composes its keys, so it sounds them, as Windows does.
   */
  static keysSilent(password: boolean, english: boolean, englishCandidates: boolean): boolean {
    return password || (english && !englishCandidates);
  }

  /**
   * The sound class of a key-down, or -1 for a key that makes no sound.
   *
   * Space, Enter and Backspace have their own samples; any other key that types a character sounds as a key. Modifier chords, modifiers on their own and navigation keys are silent: they are shortcuts and movement, not typing, and a sound on Ctrl+C would be noise.
   */
  static keyClass(
    keyCode: number,
    unicodeChar: number,
    ctrl: boolean,
    alt: boolean,
    logo: boolean,
  ): number {
    if (ctrl || alt || logo) {
      return -1;
    }
    if (keyCode === KEYCODE_SPACE) {
      return KeySoundClass.SPACE;
    }
    if (keyCode === KEYCODE_ENTER || keyCode === KEYCODE_NUMPAD_ENTER) {
      return KeySoundClass.ENTER;
    }
    if (keyCode === KEYCODE_DEL || keyCode === KEYCODE_FORWARD_DEL) {
      return KeySoundClass.BACKSPACE;
    }
    return unicodeChar > 0x20 && unicodeChar !== 0x7f ? KeySoundClass.DEFAULT : -1;
  }

  /** The files `keys` and `melody` play, each once, with every pitch it is played at. A melody sample is played at its tune's pitches; the commit sample at its own and at `tierSemitones`, the tier-up sound's pitches (empty while that sound is off); every other sample at its own. */
  static samples(
    keys: KeySoundPackFiles | null,
    melody: KeySoundPackFiles | null,
    tierSemitones: number[],
  ): KeySoundSampleRequest[] {
    const pitches: Map<string, number[]> = new Map<string, number[]>();
    const add = (file: string | null, semitones: number[]): void => {
      if (file === null || file.length === 0) {
        return;
      }
      const known: number[] = pitches.get(file) ?? [];
      for (const semitone of semitones) {
        if (known.indexOf(semitone) < 0) {
          known.push(semitone);
        }
      }
      pitches.set(file, known);
    };
    if (keys !== null) {
      const sounds: KeySoundPackSounds = keys.sounds;
      if (keys.mode === "keys") {
        add(sounds.default, [0]);
        add(sounds.space, [0]);
        add(sounds.enter, [0]);
        add(sounds.backspace, [0]);
      }
      add(sounds.commit, [0]);
      add(sounds.commit, tierSemitones);
      add(sounds.achievement, [0]);
    }
    if (melody !== null && melody.sequence !== null) {
      add(melody.sequence.sample, melody.sequence.semitones);
    }
    const requests: KeySoundSampleRequest[] = [];
    pitches.forEach((semitones: number[], file: string): void => {
      requests.push({ file: file, semitones: semitones });
    });
    return requests;
  }

  /**
   * What `event` plays: host-api's `sounds_for`. Stepping `state` is the only thing it changes.
   *
   * A key plays its class's sample, falling back to the pack's default, or with the melody on the tune's next note when the tune advances on keys. A commit plays the commit sample when that sound is on, and the next note when the tune advances on commits. An achievement plays the achievement sample. A key pack in sequence mode has no key samples, so keys stay silent with it, as they do on the desktop.
   */
  static cues(
    settings: KeySoundSettings,
    keys: KeySoundPackFiles | null,
    melody: KeySoundPackFiles | null,
    event: KeySoundEvent,
    keyClass: number,
    state: KeySoundMelody,
    now: number,
  ): KeySoundCue[] {
    const cues: KeySoundCue[] = [];
    const sequence: KeySoundPackSequence | null =
      settings.key && settings.melody && melody !== null ? melody.sequence : null;
    const note = (advance: string): void => {
      if (sequence === null || melody === null || sequence.advance !== advance) {
        return;
      }
      const semitone: number | null = state.step(
        sequence.semitones,
        now,
        melody.melody_idle_reset_millis,
      );
      if (semitone !== null) {
        cues.push({ file: sequence.sample, semitone: semitone });
      }
    };
    const plain = (file: string | null): void => {
      if (file !== null && file.length > 0) {
        cues.push({ file: file, semitone: 0 });
      }
    };
    if (event === KeySoundEvent.KEY && settings.key) {
      if (settings.melody) {
        note("key");
      } else if (keys !== null && keys.mode === "keys") {
        plain(KeySoundPolicy.keySample(keys.sounds, keyClass));
      }
    } else if (event === KeySoundEvent.COMMIT) {
      if (settings.commit && keys !== null) {
        plain(keys.sounds.commit);
      }
      note("commit");
    } else if (event === KeySoundEvent.ACHIEVEMENT && settings.achievements && keys !== null) {
      plain(keys.sounds.achievement);
    }
    return cues;
  }

  /** The tier-up sound: the key pack's commit sample at `semitone`, or null while that sound is off or the pack has no commit sample. */
  static tierCue(
    settings: KeySoundSettings,
    keys: KeySoundPackFiles | null,
    semitone: number,
  ): KeySoundCue | null {
    const file: string | null = keys === null ? null : keys.sounds.commit;
    if (!settings.tierSound || file === null || file.length === 0) {
      return null;
    }
    return { file: file, semitone: semitone };
  }

  /** The sample for a key class, or the pack's default when it has none of its own. */
  static keySample(sounds: KeySoundPackSounds, keyClass: number): string | null {
    let own: string | null = null;
    if (keyClass === KeySoundClass.SPACE) {
      own = sounds.space;
    } else if (keyClass === KeySoundClass.ENTER) {
      own = sounds.enter;
    } else if (keyClass === KeySoundClass.BACKSPACE) {
      own = sounds.backspace;
    }
    return own ?? sounds.default;
  }

  /** The key SoundPool loads a prepared file at a pitch under. */
  static cueKey(file: string, semitone: number): string {
    return `${semitone}:${file}`;
  }

  /**
   * Whether this host plays a sample: WAV only, which it decodes natively under the pack bound, both from the header and while decoding, before SoundPool sees the result. Anything else is Ogg, which only SoundPool could decode, whole and with nothing bounding how long the decoded sound is, so an Ogg sample stays silent here. The extension is the one client-core checked against the header bytes.
   */
  static isWav(file: string): boolean {
    return file.toLowerCase().endsWith(".wav");
  }

  /**
   * The manifests a pack named `id` may be read from: the built-in one under `soundPacks`, and the installed one under `stateRoot/plugins/<kind>`. host-api resolves a built-in id from the first and any other from the second; this host does not know which ids are built in, so it watches both, and the one that does not exist stamps as absent.
   *
   * @param kind "sound" for a key sound or melody pack, "music" for a music pack
   */
  static manifests(kind: string, soundPacks: string, stateRoot: string, id: string): string[] {
    const paths: string[] = [`${soundPacks}/${id}/plugin.toml`];
    if (stateRoot.length > 0) {
      paths.push(`${stateRoot}/plugins/${kind}/${id}/plugin.toml`);
    }
    return paths;
  }

  /**
   * A stamp of the manifests a loaded pack came from, as host-api stamps them (`restamp`): inode, size and modification time of each, or "-" where there is none. Importing a pack again under the same id replaces its folder whole and removing it deletes it, so either moves the stamp, and a player holding the old files reloads.
   */
  static stamp(stats: (KeySoundManifestStat | null)[]): string {
    return stats
      .map((stat: KeySoundManifestStat | null): string =>
        stat === null ? "-" : `${String(stat.ino)}:${stat.size}:${stat.mtime}`,
      )
      .join("|");
  }
}
