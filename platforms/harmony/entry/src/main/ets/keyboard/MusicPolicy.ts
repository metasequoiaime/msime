/**
 * What the 2in1 background music plays and when, decided without a device.
 *
 * The desktop hosts stream music through host-api's player (`crates/host-api/src/key_sound/player.rs`, `tick_music`), which HarmonyOS does not link; this host streams the same packs through AVPlayer instead. The rules are that player's: music plays only while switched on with a pack chosen and while the host says the input method is active in a field that is not a secure one; tracks play in the pack's order and start over after the last; a track whose length is not within the pack bound is skipped; a pack with no playable track plays nothing until the settings change. Validating a pack is not ported: `msime_client_music_pack` answers with the tracks client-core's validation accepted.
 */
import { MusicPreferenceDocument, PluginPreferenceDocument } from "./KeySoundPolicy";

/** The music settings of one preference document. */
export interface MusicSettings {
  /** Switched on with a pack chosen: a switch without a pack plays nothing, as on the desktop. */
  readonly enabled: boolean;
  readonly pack: string;
  /** 0-100. */
  readonly volume: number;
}

/** The value `msime_client_music_pack` answers with. */
export interface MusicPackTracks {
  id: string;
  name: string;
  license: string;
  /** Absolute paths, in play order. */
  tracks: string[];
  max_track_seconds: number;
}

/** What a change of settings asks of a running player. */
export enum MusicChange {
  /** Nothing the player plays by changed. */
  NONE,
  /** Only the volume: applied to the playing track, and a failed pack is tried again. */
  VOLUME,
  /** The pack, the switch or where packs are: whatever was playing is let go and the new settings start afresh. */
  RELOAD,
}

/** What the player that holds a track is sent to bring playback to what the session asks for. */
export enum MusicStep {
  /** Nothing: playback already matches, a play() or pause() sent earlier has not settled yet, or the player is between states that take neither. */
  NONE,
  PLAY,
  PAUSE,
  /** Let the player go: the system stopped it when another app took the audio focus, and a stopped player cannot play again without being prepared, so the track is opened afresh when music may play. */
  RELEASE,
}

/** `MusicPreferences::default()`'s volume. */
const DEFAULT_VOLUME: number = 30;

export const MUSIC_OFF: MusicSettings = {
  enabled: false,
  pack: "",
  volume: DEFAULT_VOLUME,
};

export class MusicPolicy {
  static metadataResultApplies(requestGeneration: number, currentGeneration: number): boolean {
    return requestGeneration === currentGeneration;
  }

  /** The settings `preferences.plugins.music` asks for; a missing record is the default, which is off. */
  static settings(plugins: PluginPreferenceDocument | undefined): MusicSettings {
    const music: MusicPreferenceDocument | undefined = plugins?.music;
    if (music === undefined) {
      return MUSIC_OFF;
    }
    const pack: string = typeof music.pack === "string" ? music.pack : "";
    const volume: number =
      typeof music.volume === "number" && music.volume >= 0 && music.volume <= 100
        ? Math.round(music.volume)
        : DEFAULT_VOLUME;
    return { enabled: music.enabled === true && pack.length > 0, pack: pack, volume: volume };
  }

  /** How `next` differs from `previous` for a player reading packs from `stateRoot`, which was `previousStateRoot`. */
  static change(
    previous: MusicSettings,
    next: MusicSettings,
    previousStateRoot: string,
    stateRoot: string,
  ): MusicChange {
    if (
      previous.enabled !== next.enabled ||
      previous.pack !== next.pack ||
      previousStateRoot !== stateRoot
    ) {
      return MusicChange.RELOAD;
    }
    return previous.volume !== next.volume ? MusicChange.VOLUME : MusicChange.NONE;
  }

  /** AVPlayer's volume, 0-1. The setting scales amplitude, as host-api's `decibels` does. */
  static gain(settings: MusicSettings): number {
    return Math.min(Math.max(settings.volume, 0), 100) / 100;
  }

  /**
   * Whether music may play now: an editor has focus and its attributes are known, it is not a secure field, and nothing is being recorded. The desktop hosts tell their player the same with `msime_client_music_set_active`.
   */
  static active(editorReady: boolean, password: boolean, recording: boolean): boolean {
    return editorReady && !password && !recording;
  }

  /**
   * Whether a track may be played: the length its container declares is known and within the pack bound, as the desktop decoder checks the declared frame count before playing. `durationMillis` is what AVMetadataExtractor reports, null when it reports nothing usable.
   */
  static trackAllowed(durationMillis: number | null, maxTrackSeconds: number): boolean {
    return (
      durationMillis !== null &&
      Number.isFinite(durationMillis) &&
      durationMillis > 0 &&
      durationMillis <= maxTrackSeconds * 1000
    );
  }

  /** AVMetadataExtractor's `duration`, a decimal count of milliseconds, as a number; null for anything else. */
  static durationMillis(duration: string | undefined): number | null {
    if (duration === undefined || !/^\d+$/.test(duration)) {
      return null;
    }
    return Number(duration);
  }

  /**
   * Whether a playing track has run past the pack bound. The declared length was checked before it started; this is the running check the desktop decoder makes on its frame count, for a file whose header understated it.
   */
  static overran(positionMillis: number, maxTrackSeconds: number): boolean {
    return positionMillis > maxTrackSeconds * 1000;
  }

  /** The track after `index`, starting over after the last. */
  static next(index: number, count: number): number {
    return count <= 0 ? 0 : (index + 1) % count;
  }
}

/**
 * Which play() or pause() the player holding a track is sent, one at a time.
 *
 * AVPlayer changes its `state` only once a play() or pause() has finished, so a decision read from `state` while one is in flight is made against the state being left: a pause sent while a play is pending reads 'paused' and is never sent, a second play() is refused as an invalid-state call. The desktop player re-checks on every tick; this one sends nothing while a command is pending and re-checks once the player settles on 'playing' or 'paused', so whatever the session said last is what playback ends up matching.
 */
export class MusicTransport {
  private pending: boolean = false;

  /** What to send a player in `state` for music that `wanted` to play or not. A PLAY or PAUSE is taken as sent. */
  step(wanted: boolean, state: string): MusicStep {
    if (state === "stopped") {
      return MusicStep.RELEASE;
    }
    if (this.pending) {
      return MusicStep.NONE;
    }
    let step: MusicStep = MusicStep.NONE;
    if (!wanted && state === "playing") {
      step = MusicStep.PAUSE;
    } else if (wanted && (state === "prepared" || state === "paused")) {
      step = MusicStep.PLAY;
    }
    this.pending = step !== MusicStep.NONE;
    return step;
  }

  /** The player reported `state`. True when that settles a command sent here, so the caller steps again against what the session says now. A pause the system made on its own is not answered with a play: music does not take the audio focus back from the app that just claimed it. */
  settled(state: string): boolean {
    if (!this.pending || (state !== "playing" && state !== "paused")) {
      return false;
    }
    this.pending = false;
    return true;
  }

  /** The player was let go, with whatever it was sent. */
  reset(): void {
    this.pending = false;
  }
}
