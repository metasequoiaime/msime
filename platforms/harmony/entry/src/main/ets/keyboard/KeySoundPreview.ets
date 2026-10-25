/**
 * 设置页「音效」里选了一个包之后的试听：经 SoundPool 播一次它的普通键样本。
 *
 * 和键盘的 `KeySoundPlayer` 走同一条路：包里有什么来自 `msime_client_key_sound_pack`，样本先由原生代码（`keySoundRenderNotes`）在包的时长上限内解码写出，SoundPool 只解码本宿主写出的文件，所以用户导入的包在这里也不会被无上限地解码。试听只有一声，播完就把声音池和写出的文件一起放掉；新的试听会先放掉上一次的。什么都不会阻塞设置页：读包和解码都在调用方这一轮之后进行，失败只记日志。
 */
import audio from '@ohos.multimedia.audio';
import media from '@ohos.multimedia.media';
import fs from '@ohos.file.fs';
import client from 'libmsimeclient.so';
import { KeyboardLog, describeError } from './KeyboardLog';
import { NativeCleanup } from './NativeCleanup';
import { NativeReplyPolicy } from './NativeReplyPolicy';
import { StagedResources } from './StagedResources';
import { AudioVolumePolicy } from './AudioVolumePolicy';
import { KeySoundPackFiles, KeySoundPolicy } from './KeySoundPolicy';
import { KeySoundPlayer } from './KeySoundPlayer';

interface KeySoundPreviewRequest {
  state_root: string;
  sound_packs: string;
  pack: string;
}

interface KeySoundPreviewResponse {
  ok: boolean;
  value: KeySoundPackFiles;
  error: string;
}

// 播完之后再留这么久才放掉声音池，让最后一点尾音不被截断。
const RELEASE_MARGIN_MILLIS: number = 500;

export class KeySoundPreview {
  /** 每次试听加一，被新试听或 `stop` 赶超的那次丢掉自己的结果。 */
  private generation: number = 0;
  private pool: media.SoundPool | undefined = undefined;
  private file: fs.File | undefined = undefined;
  private releaseTimer: number = -1;

  /**
   * @param soundPacks 安装包随带的音效包目录
   * @param stateRoot 偏好目录，用户导入的包在它的 `plugins/` 下
   * @param cacheRoot 写出音符的目录，与键盘的 `key-sounds` 分开
   * @param volume `plugins.key_sound.volume`，0–100
   */
  play(soundPacks: string, stateRoot: string, cacheRoot: string, pack: string, volume: number): void {
    this.stop();
    const generation: number = this.generation;
    setTimeout((): void => {
      this.load(generation, soundPacks, stateRoot, cacheRoot, pack, volume).catch((error: Object): void => {
        KeyboardLog.error(`key sound preview unavailable: ${describeError(error)}`);
      });
    }, 0);
  }

  /** 放掉正在播的试听、它的声音池和写出的音符。 */
  stop(): void {
    this.generation += 1;
    if (this.releaseTimer >= 0) {
      clearTimeout(this.releaseTimer);
      this.releaseTimer = -1;
    }
    const file: fs.File | undefined = this.file;
    this.file = undefined;
    if (file !== undefined) {
      NativeCleanup.close(file);
    }
    const pool: media.SoundPool | undefined = this.pool;
    this.pool = undefined;
    if (pool !== undefined) {
      pool.release().catch((error: Object): void => {
        KeyboardLog.error(`key sound preview pool would not release: ${describeError(error)}`);
      });
    }
  }

  private async load(generation: number, soundPacks: string, stateRoot: string, cacheRoot: string,
    pack: string, volume: number): Promise<void> {
    try {
      const request: KeySoundPreviewRequest = { state_root: stateRoot, sound_packs: soundPacks, pack: pack };
      const response: KeySoundPreviewResponse =
        JSON.parse(client.keySoundPack(JSON.stringify(request))) as KeySoundPreviewResponse;
      const files: KeySoundPackFiles | null = NativeReplyPolicy.successfulValue<KeySoundPackFiles>(response);
      if (files === null) {
        KeyboardLog.error(`key sound preview: pack unavailable: ${response.error}`);
        return;
      }
      const sample: string | null = KeySoundPolicy.previewSample(files);
      if (sample === null || generation !== this.generation) {
        return;
      }
      // 上一次试听写出的音符不再有用；每次从空目录开始，目录里最多只有这一个音符。
      StagedResources.removeDirectory(cacheRoot);
      const directory: string = `${cacheRoot}/${generation}`;
      fs.mkdirSync(directory, true);
      const notes: string[] = await client.keySoundRenderNotes(sample, [0], directory, files.max_sample_millis);
      if (notes.length === 0 || generation !== this.generation) {
        return;
      }
      const pool: media.SoundPool = await media.createSoundPool(1, {
        usage: audio.StreamUsage.STREAM_USAGE_MUSIC, rendererFlags: 0
      });
      if (generation !== this.generation) {
        pool.release().catch((error: Object): void => {
          KeyboardLog.error(`key sound preview pool would not release: ${describeError(error)}`);
        });
        return;
      }
      this.pool = pool;
      const gain: number = AudioVolumePolicy.gain(volume);
      const holdMillis: number = files.max_sample_millis + RELEASE_MARGIN_MILLIS;
      // loadComplete 可能早于 load() 返回，所以只用回调给出的编号，描述符也要等到这时才能关。
      pool.on('loadComplete', (sound: number): void => {
        if (generation !== this.generation) {
          return;
        }
        const file: fs.File | undefined = this.file;
        this.file = undefined;
        if (file !== undefined) {
          NativeCleanup.close(file);
        }
        if (gain <= 0) {
          this.stop();
          return;
        }
        const parameters: media.PlayParameters = {
          loop: 0, rate: audio.AudioRendererRate.RENDER_RATE_NORMAL, leftVolume: gain, rightVolume: gain,
          priority: 0
        };
        pool.play(sound, parameters).then((): void => {
          if (generation === this.generation) {
            this.releaseTimer = setTimeout((): void => {
              if (generation === this.generation) {
                this.stop();
              }
            }, holdMillis);
          }
        }).catch((error: Object): void => {
          KeyboardLog.error(`key sound preview did not play: ${describeError(error)}`);
          if (generation === this.generation) {
            this.stop();
          }
        });
      });
      pool.on('error', (error: Object): void => {
        if (generation === this.generation) {
          KeyboardLog.error(`key sound preview pool error: ${describeError(error)}`);
          this.stop();
        }
      });
      const file: fs.File = KeySoundPlayer.openPreview(notes[0]);
      this.file = file;
      try {
        await pool.load(file.fd, 0, fs.statSync(file.fd).size);
      } catch (error) {
        if (this.file === file) {
          this.file = undefined;
          NativeCleanup.close(file);
        }
        // 被新的试听赶超时 `stop` 已经关了描述符，这次加载失败是预料之中的，不必记录。
        if (generation !== this.generation) {
          return;
        }
        throw error as Error;
      }
    } catch (error) {
      KeyboardLog.error('keyboard.KeySoundPreview.load failed');
      throw error as Error;
    }
  }
}
