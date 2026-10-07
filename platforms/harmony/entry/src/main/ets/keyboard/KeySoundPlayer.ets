/**
 * Key sounds, the typing melody, commit and achievement sounds on a 2in1, played through SoundPool.
 *
 * The desktop hosts play these through host-api, which does not link an audio stack on HarmonyOS, so this host plays the same packs itself. What a pack holds comes from `msime_client_key_sound_pack`, which answers with the files client-core's validation accepted; which of them an event plays is `KeySoundPolicy`, the desktop player's rules. A WAV sample is decoded natively (`keySoundRenderNotes`, miniaudio) under the pack's length bound and written back out once per pitch it is played at, so SoundPool only ever decodes files this host wrote, and a melody's notes are pitched the way the desktop pitches them. An Ogg sample stays silent: nothing here decodes Vorbis, and SoundPool would decode it whole with nothing bounding its length.
 *
 * Nothing is created until a sound is switched on, and nothing here can stop a key from typing. A pack that will not load says so once and stays silent; SoundPool failing to play turns the sounds off until the settings change. The pool mixes with whatever else is playing rather than pausing it: SoundPool plays short sounds in mixing mode for the music stream usage, unlike the voice recording's CONCURRENCY_PAUSE_OTHERS session.
 */
import audio from '@ohos.multimedia.audio';
import media from '@ohos.multimedia.media';
import fs from '@ohos.file.fs';
import common from '@ohos.app.ability.common';
import client from 'libmsimeclient.so';
import { KeyboardLog, describeError } from './KeyboardLog';
import { StagedResources } from './StagedResources';
import {
  KEY_SOUNDS_OFF, KeySoundCue, KeySoundEvent, KeySoundManifestStat, KeySoundMelody, KeySoundPackFiles,
  KeySoundPolicy, KeySoundSampleRequest, KeySoundSelection, KeySoundSettings
} from './KeySoundPolicy';
import { TypingEffectPolicy } from './TypingEffectPolicy';
import { NativeReplyPolicy } from './NativeReplyPolicy';

interface KeySoundPackRequest {
  state_root: string;
  sound_packs: string;
  pack: string;
}

interface KeySoundPackResponse {
  ok: boolean;
  value: KeySoundPackFiles;
  error: string;
}

// Key sounds are short; eight at once covers fast typing with a melody note still ringing.
const POOL_STREAMS: number = 8;

export class KeySoundPlayer {
  private settings: KeySoundSettings = KEY_SOUNDS_OFF;
  private selection: KeySoundSelection = KeySoundPolicy.selection(KEY_SOUNDS_OFF);
  private stateRoot: string = '';
  private soundPacks: string = '';
  private cacheRoot: string = '';
  /** Moves on every reload, so a load that a newer settings change overtook throws its work away. */
  private generation: number = 0;
  private pool: media.SoundPool | undefined = undefined;
  private keys: KeySoundPackFiles | null = null;
  private melody: KeySoundPackFiles | null = null;
  private readonly melodyState: KeySoundMelody = new KeySoundMelody();
  /** Cue keys (`KeySoundPolicy.cueKey`) of the sounds SoundPool has finished loading. */
  private readonly ready: Map<string, number> = new Map<string, number>();
  /** Loaded files waiting for SoundPool's loadComplete, with the descriptor that has to stay open until then. */
  private readonly loading: Map<number, fs.File> = new Map<number, fs.File>();
  private readonly loadingCues: Map<number, string[]> = new Map<number, string[]>();
  /** loadComplete can arrive before load() resolves with the same id. */
  private readonly completed: Set<number> = new Set<number>();
  private failed: boolean = false;
  /** The manifests the loaded packs were read from, and their stamp then (`KeySoundPolicy.stamp`): a pack imported again under the same id, or removed, moves the stamp, and `recheck` reloads. Empty while nothing is loaded. */
  private manifests: string[] = [];
  private stamp: string = '';

  /** Where the built-in packs are and where rendered notes may be written. */
  use(context: common.Context): void {
    this.soundPacks = `${context.resourceDir}/sound-packs`;
    this.cacheRoot = `${context.cacheDir}/key-sounds`;
  }

  /**
   * Take the sound settings of a newly read preference document. Only a change of pack reloads anything; the volume is read at every sound.
   *
   * @param stateRoot the preferences directory, which holds the installed packs under `plugins/`
   */
  configure(settings: KeySoundSettings, stateRoot: string): void {
    const selection: KeySoundSelection = KeySoundPolicy.selection(settings);
    // After a play failure the pool is gone, so any settings change loads it again, as the failure message promises.
    const reload: boolean = this.failed || selection.pack !== this.selection.pack
      || selection.melodyPack !== this.selection.melodyPack || selection.tierSound !== this.selection.tierSound
      || stateRoot !== this.stateRoot;
    this.settings = settings;
    if (!reload) {
      return;
    }
    this.selection = selection;
    this.stateRoot = stateRoot;
    this.reload();
  }

  /**
   * Reload the packs when one of them changed on disk since it was loaded: imported again under the same id, which replaces its folder whole, or removed. The session calls this when an editor takes focus, as host-api re-stamps its packs on focus; the stat runs off the caller's turn.
   */
  recheck(): void {
    if (this.manifests.length === 0) {
      return;
    }
    const generation: number = this.generation;
    setTimeout((): void => {
      if (generation === this.generation && this.manifests.length > 0
        && KeySoundPlayer.stampOf(this.manifests) !== this.stamp) {
        this.reload();
      }
    }, 0);
  }

  /** Let whatever is loaded go and load the current selection afresh. Ends a failure. */
  private reload(): void {
    this.release();
    this.failed = false;
    this.manifests = [];
    this.stamp = '';
    if (!KeySoundPolicy.wanted(this.settings) || this.soundPacks.length === 0) {
      return;
    }
    const generation: number = this.generation;
    // Off the caller's turn: reading a pack touches the disk, and this runs while the keyboard is starting.
    setTimeout((): void => {
      this.load(generation).catch((error: Object): void => {
        KeyboardLog.error(`key sounds unavailable: ${describeError(error)}`);
      });
    }, 0);
  }

  /** A key went down; `keyClass` is a `KeySoundClass`. */
  key(keyClass: number): void {
    this.play(KeySoundEvent.KEY, keyClass);
  }

  /** Text was committed. */
  commit(): void {
    this.play(KeySoundEvent.COMMIT, 0);
  }

  /** The commit count just passed a milestone. */
  achievement(): void {
    this.play(KeySoundEvent.ACHIEVEMENT, 0);
  }

  /** The typing combo reached `tier` and host-api reported its tier-up sound due: the key pack's commit sample, raised per tier. */
  tierUp(tier: number): void {
    const pool: media.SoundPool | undefined = this.pool;
    if (pool === undefined || this.failed) {
      return;
    }
    const cue: KeySoundCue | null =
      KeySoundPolicy.tierCue(this.settings, this.keys, TypingEffectPolicy.tierSemitone(tier));
    if (cue !== null) {
      this.sound(pool, [cue]);
    }
  }

  /** Let go of the pool, its sounds and the rendered notes. The settings stay, so the next configure with a different pack loads again. */
  release(): void {
    this.generation += 1;
    this.melodyState.reset();
    this.ready.clear();
    this.loadingCues.clear();
    this.completed.clear();
    this.loading.forEach((file: fs.File): void => {
      fs.closeSync(file);
    });
    this.loading.clear();
    this.keys = null;
    this.melody = null;
    const pool: media.SoundPool | undefined = this.pool;
    this.pool = undefined;
    if (pool !== undefined) {
      pool.release().catch((error: Object): void => {
        KeyboardLog.error(`key sound pool would not release: ${describeError(error)}`);
      });
    }
    this.clearNotes();
  }

  /** Stop for good: the ability is going away. */
  shutdown(): void {
    this.release();
    this.settings = KEY_SOUNDS_OFF;
    this.selection = KeySoundPolicy.selection(KEY_SOUNDS_OFF);
    this.manifests = [];
    this.stamp = '';
  }

  private play(event: KeySoundEvent, keyClass: number): void {
    const pool: media.SoundPool | undefined = this.pool;
    if (pool === undefined || this.failed) {
      return;
    }
    const cues: KeySoundCue[] = KeySoundPolicy.cues(this.settings, this.keys, this.melody, event,
      keyClass, this.melodyState, Date.now());
    this.sound(pool, cues);
  }

  /** Play each cue SoundPool has finished loading, at the current volume. */
  private sound(pool: media.SoundPool, cues: KeySoundCue[]): void {
    const gain: number = KeySoundPolicy.gain(this.settings);
    if (gain <= 0) {
      return;
    }
    for (const cue of cues) {
      const sound: number | undefined = this.ready.get(KeySoundPolicy.cueKey(cue.file, cue.semitone));
      if (sound === undefined) {
        continue;
      }
      const parameters: media.PlayParameters = {
        loop: 0, rate: audio.AudioRendererRate.RENDER_RATE_NORMAL, leftVolume: gain, rightVolume: gain,
        priority: 0
      };
      pool.play(sound, parameters).catch((error: Object): void => {
        this.fail(pool, error);
      });
    }
  }

  private fail(pool: media.SoundPool, error: Object): void {
    if (pool !== this.pool || this.failed) {
      return;
    }
    this.failed = true;
    KeyboardLog.error(`key sounds off until the settings change: ${describeError(error)}`);
    this.release();
  }

  private async load(generation: number): Promise<void> {
    if (generation !== this.generation) {
      this.clearGeneration(generation);
      return;
    }
    // Stamped before the packs are read, so a change while they are read is seen by the next recheck rather than missed.
    const manifests: string[] = this.manifestPaths();
    this.manifests = manifests;
    this.stamp = KeySoundPlayer.stampOf(manifests);
    const keys: KeySoundPackFiles | null =
      this.selection.pack === null ? null : this.packFiles(this.selection.pack);
    const melody: KeySoundPackFiles | null =
      this.selection.melodyPack === null ? null : this.packFiles(this.selection.melodyPack);
    if ((this.selection.pack !== null && keys === null)
      || (this.selection.melodyPack !== null && melody === null)) {
      return;
    }
    if (melody !== null && melody.sequence === null) {
      KeyboardLog.error(`sound pack ${melody.id}: not a melody`);
      return;
    }
    const pool: media.SoundPool = await media.createSoundPool(POOL_STREAMS, {
      usage: audio.StreamUsage.STREAM_USAGE_MUSIC, rendererFlags: 0
    });
    if (generation !== this.generation) {
      pool.release().catch((error: Object): void => {
        KeyboardLog.error(`key sound pool would not release: ${describeError(error)}`);
      });
      return;
    }
    this.pool = pool;
    this.keys = keys;
    this.melody = melody;
    pool.on('loadComplete', (sound: number): void => {
      if (generation === this.generation) {
        this.completed.add(sound);
        this.settle(sound);
      }
    });
    // A sample the pool cannot decode is reported here; it costs that one sound, not the others. A play that fails is what turns the sounds off.
    pool.on('error', (error: Object): void => {
      if (generation === this.generation) {
        KeyboardLog.error(`key sound pool error: ${describeError(error)}`);
      }
    });
    const requests: KeySoundSampleRequest[] = KeySoundPolicy.samples(keys, melody,
      this.selection.tierSound ? TypingEffectPolicy.tierSemitones() : []);
    const bound: KeySoundPackFiles | null = keys !== null ? keys : melody;
    const maxMillis: number = bound === null ? 0 : bound.max_sample_millis;
    for (let index: number = 0; index < requests.length; index++) {
      const directory: string = `${this.cacheRoot}/${generation}/${index}`;
      fs.mkdirSync(directory, true);
      await this.prepare(generation, pool, requests[index], directory, maxMillis);
      if (generation !== this.generation) {
        this.clearGeneration(generation);
        return;
      }
    }
  }

  /** The manifests of the selected packs, wherever host-api may read them from. */
  private manifestPaths(): string[] {
    let paths: string[] = [];
    if (this.selection.pack !== null) {
      paths = paths.concat(KeySoundPolicy.manifests('sound', this.soundPacks, this.stateRoot,
        this.selection.pack));
    }
    if (this.selection.melodyPack !== null) {
      paths = paths.concat(KeySoundPolicy.manifests('sound', this.soundPacks, this.stateRoot,
        this.selection.melodyPack));
    }
    return paths;
  }

  /** The stamp of `paths` now; a manifest that cannot be read stamps as absent. `MusicPlayer` stamps its pack the same way. */
  static stampOf(paths: string[]): string {
    const stats: (KeySoundManifestStat | null)[] = [];
    for (const path of paths) {
      let stat: KeySoundManifestStat | null = null;
      try {
        const found: fs.Stat = fs.lstatSync(path);
        stat = { ino: found.ino, size: found.size, mtime: found.mtime };
      } catch (error) {
        // A manifest that is not there is part of the stamp: the pack may be installed later.
        stat = null;
      }
      stats.push(stat);
    }
    return KeySoundPolicy.stamp(stats);
  }

  /** One pack's validated files, or null after saying why there are none. */
  private packFiles(pack: string): KeySoundPackFiles | null {
    const request: KeySoundPackRequest = {
      state_root: this.stateRoot, sound_packs: this.soundPacks, pack: pack
    };
    const response: KeySoundPackResponse =
      JSON.parse(client.keySoundPack(JSON.stringify(request))) as KeySoundPackResponse;
    const value: KeySoundPackFiles | null = NativeReplyPolicy.successfulValue<KeySoundPackFiles>(response);
    if (value === null) {
      KeyboardLog.error(`sound pack unavailable: ${response.error}`);
      return null;
    }
    return value;
  }

  /** Render one sample and hand its notes to SoundPool. A sample that will not prepare leaves that one sound silent and says why. */
  private async prepare(generation: number, pool: media.SoundPool, request: KeySoundSampleRequest,
    directory: string, maxMillis: number): Promise<void> {
    try {
      if (!KeySoundPolicy.isWav(request.file)) {
        KeyboardLog.error('sound pack sample refused: HarmonyOS plays WAV samples only');
        return;
      }
      const notes: string[] =
        await client.keySoundRenderNotes(request.file, request.semitones, directory, maxMillis);
      for (let index: number = 0; index < notes.length && generation === this.generation; index++) {
        await this.loadFile(generation, pool, notes[index],
          [KeySoundPolicy.cueKey(request.file, request.semitones[index])]);
      }
    } catch (error) {
      KeyboardLog.error(`sound pack sample unavailable: ${describeError(error)}`);
    }
  }

  /** Open a rendered note and bind the descriptor to the path produced by the renderer. */
  private static openVerified(path: string): fs.File {
    const pathStat: fs.Stat = fs.lstatSync(path);
    if (pathStat.isSymbolicLink() || !pathStat.isFile()) {
      throw new Error('rendered sound is not a regular file');
    }
    let file: fs.File | undefined = undefined;
    try {
      file = fs.openSync(path, fs.OpenMode.READ_ONLY);
      const opened: fs.Stat = fs.statSync(file.fd);
      if (!opened.isFile() || opened.ino !== pathStat.ino) {
        throw new Error('rendered sound changed while opening');
      }
      return file;
    } catch (error) {
      if (file !== undefined) fs.closeSync(file);
      throw error;
    }
  }

  private async loadFile(generation: number, pool: media.SoundPool, path: string,
    cues: string[]): Promise<void> {
    const file: fs.File = KeySoundPlayer.openVerified(path);
    let sound: number = 0;
    let loaded: boolean = false;
    try {
      sound = await pool.load(file.fd, 0, fs.statSync(file.fd).size);
      loaded = true;
    } finally {
      if (!loaded) {
        fs.closeSync(file);
      }
    }
    if (generation !== this.generation) {
      fs.closeSync(file);
      return;
    }
    this.loading.set(sound, file);
    this.loadingCues.set(sound, cues);
    this.settle(sound);
  }

  /** Publish a sound once SoundPool has both named it and finished loading it, and let its descriptor go. */
  private settle(sound: number): void {
    const cues: string[] | undefined = this.loadingCues.get(sound);
    const file: fs.File | undefined = this.loading.get(sound);
    if (cues === undefined || file === undefined || !this.completed.has(sound)) {
      return;
    }
    for (const cue of cues) {
      this.ready.set(cue, sound);
    }
    this.loadingCues.delete(sound);
    this.loading.delete(sound);
    this.completed.delete(sound);
    fs.closeSync(file);
  }

  /** Rendered notes belong to one load; the next load, or none, starts from an empty directory. */
  private clearNotes(): void {
    if (this.cacheRoot.length === 0) {
      return;
    }
    try {
      StagedResources.removeDirectory(this.cacheRoot);
    } catch (error) {
      KeyboardLog.error(`rendered key sounds not removed: ${describeError(error)}`);
    }
  }

  /** A renderer that was already in flight when release() ran can recreate its generation after the root was removed. */
  private clearGeneration(generation: number): void {
    if (this.cacheRoot.length === 0) return;
    try {
      StagedResources.removeDirectory(`${this.cacheRoot}/${generation}`);
    } catch (error) {
      KeyboardLog.error(`stale rendered key sounds not removed: ${describeError(error)}`);
    }
  }
}
