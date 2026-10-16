import { NativeCleanup } from './NativeCleanup';
/**
 * 二合一设备上的背景音乐，经 AVPlayer 流式播放。
 *
 * 桌面宿主经 host-api 流式播放音乐，而 host-api 在 HarmonyOS 上不链接音频栈，所以这个宿主自己流式播放同样的音乐包。一个包有哪些曲目由 `msime_client_music_pack` 给出，它回答的是 client-services 校验通过的文件；音乐何时可以播放、按什么顺序播放由 `MusicPolicy` 决定，也就是桌面播放器的规则。AVPlayer 从文件流式读取而不是整个解码，所以一首曲目占用的是解码器的缓冲区，而不是与时长相当的内存，并且这里除了 WAV 也能播放 Ogg 曲目。曲目开始前用 AVMetadataExtractor 读出它声明的时长，与包的上限核对；播放过程中再用同一个上限核对播放位置，就像桌面解码器核对帧数那样。
 *
 * 在音乐打开、并且一个可能听到它的编辑框获得焦点之前，什么都不创建。包载入失败、包里没有一首时长允许的曲目，或者 AVPlayer 出错，都会关闭音乐直到设置变化，并在日志里留下一行；这里的任何事都不会让按键打不出字。
 */
import audio from '@ohos.multimedia.audio';
import media from '@ohos.multimedia.media';
import fs from '@ohos.file.fs';
import common from '@ohos.app.ability.common';
import client from 'libmsimeclient.so';
import { KeyboardLog, describeError } from './KeyboardLog';
import { KeySoundPolicy } from './KeySoundPolicy';
import { NativeReplyPolicy } from './NativeReplyPolicy';
import { KeySoundPlayer } from './KeySoundPlayer';
import {
  MUSIC_OFF, MusicChange, MusicPackTracks, MusicPolicy, MusicSettings, MusicStep, MusicTransport
} from './MusicPolicy';

interface MusicPackRequest {
  state_root: string;
  sound_packs: string;
  pack: string;
}

interface MusicPackResponse {
  ok: boolean;
  value: MusicPackTracks;
  error: string;
}

export class MusicPlayer {
  private settings: MusicSettings = MUSIC_OFF;
  private stateRoot: string = '';
  private soundPacks: string = '';
  /** What the session last said: an editor that may hear music has focus. */
  private active: boolean = false;
  /** Set by a failure, cleared by the next change of the music settings. */
  private failed: boolean = false;
  /** Moves whenever the playing track is let go, so a callback or an opening that a newer change overtook does nothing. */
  private generation: number = 0;
  /** A track is being checked or opened, so another activation does not open a second one. */
  private opening: boolean = false;
  private pack: MusicPackTracks | null = null;
  private track: number = 0;
  /** Whether each track's declared length is within the pack bound, by path, for the pack in `pack`. */
  private readonly checked: Map<string, boolean> = new Map<string, boolean>();
  private player: media.AVPlayer | undefined = undefined;
  /** The play() or pause() `player` was sent and has not settled yet. */
  private readonly transport: MusicTransport = new MusicTransport();
  /** The playing track's descriptor, which has to stay open until its player is released. */
  private file: fs.File | undefined = undefined;
  /** The manifests the pack was resolved from, and their stamp then (`KeySoundPolicy.stamp`): a pack imported again under the same id, or removed, moves the stamp, and `recheck` starts afresh. Empty until a pack is resolved. */
  private manifests: string[] = [];
  private stamp: string = '';

  /** Where the built-in packs are, for the request that names them: the built-in music packs sit beside the built-in sound packs, and any other is installed under the state root. */
  use(context: common.Context): void {
    this.soundPacks = `${context.resourceDir}/sound-packs`;
  }

  /**
   * Take the music settings of a newly read preference document. A new pack, a switch turned or a new state root starts afresh; a new volume is applied to the playing track. Any change ends a failure.
   *
   * @param stateRoot the preferences directory, which holds the installed packs under `plugins/`
   */
  configure(settings: MusicSettings, stateRoot: string): void {
    const change: MusicChange = MusicPolicy.change(this.settings, settings, this.stateRoot, stateRoot);
    this.settings = settings;
    this.stateRoot = stateRoot;
    if (change === MusicChange.NONE) {
      return;
    }
    if (change === MusicChange.RELOAD || this.failed) {
      this.forget();
    } else if (this.player !== undefined && MusicPlayer.audible(this.player.state)) {
      this.player.setVolume(MusicPolicy.gain(settings));
    }
    this.failed = false;
    this.update();
  }

  /**
   * Start afresh when the pack changed on disk since it was resolved: imported again under the same id, which replaces its folder whole, or removed. The session calls this when an editor takes focus, as host-api re-stamps its packs on focus; the stat runs off the caller's turn. A pack that failed to resolve is tried again once it changes.
   */
  recheck(): void {
    if (this.manifests.length === 0) {
      return;
    }
    setTimeout((): void => {
      if (this.manifests.length > 0 && KeySoundPlayer.stampOf(this.manifests) !== this.stamp) {
        this.forget();
        this.failed = false;
        this.update();
      }
    }, 0);
  }

  /** Let the playing track go and forget the pack, so the next start resolves it again. */
  private forget(): void {
    this.release();
    this.pack = null;
    this.track = 0;
    this.checked.clear();
    this.manifests = [];
    this.stamp = '';
  }

  /** Whether music may play now; see `MusicPolicy.active`. */
  setActive(active: boolean): void {
    if (active === this.active) {
      return;
    }
    this.active = active;
    this.update();
  }

  /** Stop for good: the ability is going away. */
  shutdown(): void {
    this.release();
    this.settings = MUSIC_OFF;
    this.active = false;
    this.pack = null;
    this.track = 0;
    this.checked.clear();
    this.manifests = [];
    this.stamp = '';
  }

  /** The states AVPlayer takes a volume in. */
  private static audible(state: media.AVPlayerState): boolean {
    return state === 'prepared' || state === 'playing' || state === 'paused' || state === 'completed';
  }

  /** Bring playback to what the settings and the session ask for: paused while music may not play, playing or being opened while it may. */
  private update(): void {
    const player: media.AVPlayer | undefined = this.player;
    const wanted: boolean = this.active && this.settings.enabled && !this.failed;
    if (player !== undefined) {
      const step: MusicStep = this.transport.step(wanted, player.state);
      if (step === MusicStep.PAUSE) {
        // Pausing is what keeps music out of a password field, so a pause that fails lets the player go instead.
        player.pause().catch((error: Object): void => {
          this.fail(player, error);
        });
      } else if (step === MusicStep.PLAY) {
        player.play().catch((error: Object): void => {
          this.fail(player, error);
        });
      }
      if (step !== MusicStep.RELEASE) {
        return;
      }
      this.release();
    }
    if (!wanted || this.opening || this.soundPacks.length === 0) {
      return;
    }
    this.opening = true;
    const generation: number = this.generation;
    this.start(generation).catch((error: Object): void => {
      if (generation === this.generation) {
        this.stop(`music off until the settings change: ${describeError(error)}`);
      }
    });
  }

  /** Resolve the pack if it is not yet, then open the current track or the first one after it whose length is allowed. */
  private async start(generation: number): Promise<void> {
    if (this.pack === null) {
      this.pack = this.resolve();
    }
    const pack: MusicPackTracks | null = this.pack;
    if (pack === null) {
      return;
    }
    for (let attempt: number = 0; attempt < pack.tracks.length; attempt++) {
      const allowed: boolean = await this.allowed(pack, this.track, generation);
      if (generation !== this.generation) {
        return;
      }
      if (allowed) {
        await this.open(generation, pack, this.track);
        return;
      }
      this.track = MusicPolicy.next(this.track, pack.tracks.length);
    }
    this.stop(`music off until the settings change: music pack ${pack.id}: no track could be played`);
  }

  /** The selected pack's validated tracks, or null after turning music off with the reason. */
  private resolve(): MusicPackTracks | null {
    // Stamped before the pack is read, so a change while it is read is seen by the next recheck rather than missed.
    this.manifests = KeySoundPolicy.manifests('music', this.soundPacks, this.stateRoot, this.settings.pack);
    this.stamp = KeySoundPlayer.stampOf(this.manifests);
    const request: MusicPackRequest = {
      state_root: this.stateRoot, sound_packs: this.soundPacks, pack: this.settings.pack
    };
    const response: MusicPackResponse =
      JSON.parse(client.musicPack(JSON.stringify(request))) as MusicPackResponse;
    const value: MusicPackTracks | null = NativeReplyPolicy.successfulValue<MusicPackTracks>(response);
    if (value === null) {
      this.stop(`music off until the settings change: ${response.error}`);
      return null;
    }
    this.checked.clear();
    return value;
  }

  /** Open a validated pack track and bind the descriptor to the path checked above. */
  private static openVerified(path: string): fs.File {
    try {
      const pathStat: fs.Stat = fs.lstatSync(path);
      if (pathStat.isSymbolicLink() || !pathStat.isFile()) {
        throw new Error('music track is not a regular file');
      }
      let file: fs.File | undefined = undefined;
      try {
        file = fs.openSync(path, fs.OpenMode.READ_ONLY);
        const opened: fs.Stat = fs.statSync(file.fd);
        if (!opened.isFile() || opened.ino !== pathStat.ino) {
          throw new Error('music track changed while opening');
        }
        return file;
      } catch (error) {
        if (file !== undefined) NativeCleanup.close(file);
        // ArkTS 只允许抛 Error（arkts-limited-throw）；原样抛出同一个对象，调用方仍能读到它的错误码。
        throw error as Error;
      }
    } catch (error) {
      KeyboardLog.error('keyboard.MusicPlayer.openVerified failed');
      throw error as Error;
    }
  }

  /** Whether a track's declared length is within the pack bound. Read once per track and pack; a track that cannot be read is not played. */
  private async allowed(pack: MusicPackTracks, index: number, generation: number): Promise<boolean> {
    const path: string = pack.tracks[index];
    const known: boolean | undefined = this.checked.get(path);
    if (known !== undefined) {
      return known;
    }
    let allowed: boolean = false;
    let file: fs.File | undefined = undefined;
    let extractor: media.AVMetadataExtractor | undefined = undefined;
    try {
      file = MusicPlayer.openVerified(path);
      extractor = await media.createAVMetadataExtractor();
      const descriptor: media.AVFileDescriptor = { fd: file.fd };
      extractor.fdSrc = descriptor;
      const metadata: media.AVMetadata = await extractor.fetchMetadata();
      allowed = MusicPolicy.trackAllowed(MusicPolicy.durationMillis(metadata.duration),
        pack.max_track_seconds);
      if (!allowed) {
        KeyboardLog.error(`music track skipped: its length is unknown or over ${pack.max_track_seconds} s`);
      }
    } catch (error) {
      KeyboardLog.error(`music track skipped: ${describeError(error)}`);
    } finally {
      if (extractor !== undefined) {
        await extractor.release().catch((error: Object): void => {
          KeyboardLog.error(`music metadata reader would not release: ${describeError(error)}`);
        });
      }
      if (file !== undefined) {
        NativeCleanup.close(file);
      }
    }
    if (MusicPolicy.metadataResultApplies(generation, this.generation)) {
      this.checked.set(path, allowed);
    }
    return allowed;
  }

  /** Hand one track to a new AVPlayer. It plays once prepared if music may play then; `update` plays it later otherwise. */
  private async open(generation: number, pack: MusicPackTracks, index: number): Promise<void> {
    try {
      const player: media.AVPlayer = await media.createAVPlayer();
      if (generation !== this.generation) {
        MusicPlayer.dispose(player, undefined);
        return;
      }
      // 打开文件前先持有播放器，以便文件打开失败时随错误一起释放。
      this.player = player;
      this.opening = false;
      const file: fs.File = MusicPlayer.openVerified(pack.tracks[index]);
      this.file = file;
      const maxTrackSeconds: number = pack.max_track_seconds;
      player.on('stateChange', (state: media.AVPlayerState): void => {
        if (player !== this.player) {
          return;
        }
        if (state === 'initialized') {
          const renderer: audio.AudioRendererInfo = {
            usage: audio.StreamUsage.STREAM_USAGE_MUSIC, rendererFlags: 0
          };
          player.audioRendererInfo = renderer;
          player.prepare().catch((error: Object): void => {
            this.fail(player, error);
          });
        } else if (state === 'prepared') {
          player.setVolume(MusicPolicy.gain(this.settings));
          this.update();
        } else if (state === 'playing' || state === 'paused') {
          // `play()` 或 `pause()` 执行期间会话状态可能改变，完成后按最新状态同步播放。
          if (this.transport.settled(state)) {
            this.update();
          }
        } else if (state === 'stopped') {
          // 其他应用抢占音频焦点后立即释放播放器及描述符；下次激活时再打开曲目，避免立刻抢回焦点。
          this.release();
        } else if (state === 'completed') {
          this.advance(player);
        }
      });
      player.on('timeUpdate', (position: number): void => {
        if (player === this.player && MusicPolicy.overran(position, maxTrackSeconds)) {
          KeyboardLog.error('music track ran past the pack bound; playing the next');
          this.advance(player);
        }
      });
      player.on('error', (error: Object): void => {
        this.fail(player, error);
      });
      const descriptor: media.AVFileDescriptor = { fd: file.fd };
      player.fdSrc = descriptor;
    } catch (error) {
      KeyboardLog.error('keyboard.MusicPlayer.open failed');
      throw error as Error;
    }
  }

  /** The track ended, or ran past the bound: the next one, starting over after the last. */
  private advance(player: media.AVPlayer): void {
    const pack: MusicPackTracks | null = this.pack;
    if (player !== this.player || pack === null) {
      return;
    }
    this.track = MusicPolicy.next(this.track, pack.tracks.length);
    this.release();
    this.update();
  }

  private fail(player: media.AVPlayer, error: Object): void {
    if (player !== this.player) {
      return;
    }
    this.stop(`music off until the settings change: ${describeError(error)}`);
  }

  /** Turn music off until the settings change, saying why once. */
  private stop(reason: string): void {
    this.failed = true;
    KeyboardLog.error(reason);
    this.release();
  }

  /** Let the playing track go. The pack, the position in it and the checked lengths stay for the next track. */
  private release(): void {
    this.generation += 1;
    this.opening = false;
    this.transport.reset();
    const player: media.AVPlayer | undefined = this.player;
    const file: fs.File | undefined = this.file;
    this.player = undefined;
    this.file = undefined;
    if (player !== undefined) {
      MusicPlayer.dispose(player, file);
    }
  }

  /** Release a player, then close the descriptor it was reading, if it had one. */
  private static dispose(player: media.AVPlayer, file: fs.File | undefined): void {
    try {
      const close = (): void => {
        if (file !== undefined) {
          NativeCleanup.close(file);
        }
      };
      player.release().then(close, (error: Object): void => {
        KeyboardLog.error(`music player would not release: ${describeError(error)}`);
        close();
      });
    } catch (error) {
      KeyboardLog.error('keyboard.MusicPlayer.dispose failed');
      if (file !== undefined) NativeCleanup.close(file);
    }
  }
}
