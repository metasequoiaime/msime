/**
 * Background music on a 2in1, streamed through AVPlayer.
 *
 * The desktop hosts stream music through host-api, which does not link an audio stack on HarmonyOS, so this host streams the same packs itself. Which tracks a pack has comes from `msime_client_music_pack`, which answers with the files client-core's validation accepted; when music may play and in what order is `MusicPolicy`, the desktop player's rules. AVPlayer streams from the file rather than decoding it whole, so a track costs a decoder's buffers, not its length in memory, and Ogg tracks play here as well as WAV. Before a track starts its declared length is read with AVMetadataExtractor and checked against the pack bound, and while it plays its position is checked against the same bound, as the desktop decoder checks its frame count.
 *
 * Nothing is created until music is switched on and an editor that may hear it has focus. A pack that will not load, a pack with no track of an allowed length, or AVPlayer failing turns music off until the settings change, with one line in the log; nothing here can stop a key from typing.
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
      const allowed: boolean = await this.allowed(pack, this.track);
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

  /** Whether a track's declared length is within the pack bound. Read once per track and pack; a track that cannot be read is not played. */
  private async allowed(pack: MusicPackTracks, index: number): Promise<boolean> {
    const path: string = pack.tracks[index];
    const known: boolean | undefined = this.checked.get(path);
    if (known !== undefined) {
      return known;
    }
    let allowed: boolean = false;
    let file: fs.File | undefined = undefined;
    let extractor: media.AVMetadataExtractor | undefined = undefined;
    try {
      file = fs.openSync(path, fs.OpenMode.READ_ONLY);
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
        fs.closeSync(file);
      }
    }
    this.checked.set(path, allowed);
    return allowed;
  }

  /** Hand one track to a new AVPlayer. It plays once prepared if music may play then; `update` plays it later otherwise. */
  private async open(generation: number, pack: MusicPackTracks, index: number): Promise<void> {
    const player: media.AVPlayer = await media.createAVPlayer();
    if (generation !== this.generation) {
      MusicPlayer.dispose(player, undefined);
      return;
    }
    // Held before the file is opened, so a track that cannot be opened lets this player go with the failure it reports.
    this.player = player;
    this.opening = false;
    const file: fs.File = fs.openSync(pack.tracks[index], fs.OpenMode.READ_ONLY);
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
        // The session may have changed its mind while a play() or pause() was in flight; playback is brought to what it says now.
        if (this.transport.settled(state)) {
          this.update();
        }
      } else if (state === 'stopped') {
        // Another app took the audio focus and the system stopped this player. Letting it go frees its descriptor at once; the track is opened afresh on the next activation rather than now, so music does not take the focus straight back.
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
    const close = (): void => {
      if (file !== undefined) {
        fs.closeSync(file);
      }
    };
    player.release().then(close, (error: Object): void => {
      KeyboardLog.error(`music player would not release: ${describeError(error)}`);
      close();
    });
  }
}
