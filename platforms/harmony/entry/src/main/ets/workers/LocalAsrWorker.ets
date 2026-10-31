import { KeyboardLog } from '../keyboard/KeyboardLog';
import { ErrorEvent, MessageEvents, ThreadWorkerGlobalScope, worker } from '@kit.ArkTS';
import fs from '@ohos.file.fs';
import util from '@ohos.util';
import {
  OfflineRecognizer,
  OfflineRecognizerConfig,
  OfflineStream,
  OnlineRecognizer,
  OnlineRecognizerConfig,
  OnlineStream,
  SileroVadConfig,
  SpeechSegment,
  TenVadConfig,
  Vad,
  VadConfig
} from 'sherpa_onnx';
import {
  LOCAL_ASR_SAMPLE_RATE,
  LOCAL_MODEL_MANIFEST,
  LOCAL_MODEL_OFFLINE_FUNASR_NANO,
  LOCAL_MODEL_OFFLINE_SENSE_VOICE,
  LOCAL_MODEL_OFFLINE_TRANSDUCER,
  LOCAL_MODEL_ONLINE_TRANSDUCER,
  LOCAL_ONLINE_TAIL_SAMPLES,
  LOCAL_VAD_HISTORY_SLACK,
  LOCAL_VAD_WINDOW,
  LocalAsrPolicy,
  LocalAsrReply,
  LocalAsrRequest,
  LocalModelPlan,
  OFFLINE_TRANSDUCER_MAX_ACTIVE_PATHS,
  TRANSDUCER_HOTWORDS_SCORE
} from '../keyboard/input/LocalAsrPolicy';
import { LocalAsrPathTrust, PathTrustStat } from '../keyboard/input/LocalAsrPathTrust';
import { LocalAsrTextFile, LocalAsrTextReader, LocalAsrTextReaderApi } from '../keyboard/input/LocalAsrTextReader';
import { utf8Length } from '../keyboard/Utf8';

/**
 * The on-device recognizer, off the keyboard's UI thread.
 *
 * Loading a model takes seconds and decoding a VAD segment takes a noticeable fraction of one, so neither may run where key presses are handled. This worker owns the sherpa-onnx objects (they are bound to the thread that created them), keeps one recognizer loaded between sessions, and is terminated by the main thread when idle, which is what returns the model's memory. Audio arrives as transferred PCM16 buffers and is never written anywhere.
 */
const workerPort: ThreadWorkerGlobalScope = worker.workerPort;

class LoadedModel {
  key: string = '';
  plan: LocalModelPlan;
  online: OnlineRecognizer | undefined = undefined;
  offline: OfflineRecognizer | undefined = undefined;

  constructor(plan: LocalModelPlan) {
    this.plan = plan;
  }
}

class Session {
  id: number;
  model: LoadedModel;
  onlineStream: OnlineStream | undefined = undefined;
  vad: Vad | undefined = undefined;
  /** 送进 VAD 的最近一段录音，给段首往前补（`LocalAsrPolicy.vadSegmentStart`）。前 `historyLength` 个有效，`history[0]` 是会话里第 `historyStart` 个采样；最多留 `historyLimit` 个，sherpa-onnx 自己的缓冲只留到段首前 2 个窗口。 */
  history: Float32Array = new Float32Array(0);
  historyLength: number = 0;
  historyStart: number = 0;
  historyLimit: number = 0;
  /** 上一段的段尾。往前补不越过它，否则两段的识别结果会重复同一段音频里的字。 */
  decodedEnd: number = 0;
  segments: string[] = [];
  current: string = '';
  lastPartial: string = '';
  /** 上一次告诉主线程的 VAD 说话状态。 */
  speaking: boolean = false;

  constructor(id: number, model: LoadedModel) {
    this.id = id;
    this.model = model;
  }
}

/**
 * 补上原生层读取、HAR 却拼错了的 `hotwordsFile`。HAR 的 `OfflineRecognizerConfig` 只有 `hotwordsFfile`，原生的 `GetOfflineRecognizerConfig` 按名字读 `hotwordsFile`，所以离线识别器的热词文件只能经这个子类上的同名字段传进去。原因见 `LocalAsrPolicy.hotwordsFile`。
 */
class OfflineHotwordsRecognizerConfig extends OfflineRecognizerConfig {
  public hotwordsFile: string = '';
}

let loaded: LoadedModel | undefined = undefined;
let session: Session | undefined = undefined;

function reply(type: string, id: number, text: string, correct: boolean, kind: string = '',
  speaking: boolean = false): void {
  const message: LocalAsrReply = {
    type: type, id: id, text: text, correct: correct, kind: kind, speaking: speaking
  };
  try {
    workerPort.postMessage(message);
  } catch {
    KeyboardLog.error('local ASR reply could not be delivered');
  }
}

const textReaderApi: LocalAsrTextReaderApi = {
  open: (path: string, mode: number): LocalAsrTextFile => {
    const expected: fs.Stat = fs.lstatSync(path);
    if (expected.isSymbolicLink() || !expected.isFile()) {
      throw new Error('本地语音模型文本文件不是普通文件');
    }
    let file: fs.File | undefined = undefined;
    try {
      file = fs.openSync(path, mode);
      const opened: fs.Stat = fs.statSync(file.fd);
      if (!opened.isFile() || opened.ino !== expected.ino) {
        throw new Error('本地语音模型文本文件发生变化');
      }
      const handle: LocalAsrTextFile = { fd: file.fd };
      return handle;
    } catch (error) {
      if (file !== undefined) fs.closeSync(file);
      // ArkTS 只允许抛 Error（arkts-limited-throw）；原样抛出同一个对象，调用方仍能读到它的错误码。
      throw error as Error;
    }
  },
  read: (fd: number, buffer: ArrayBuffer): number => fs.readSync(fd, buffer),
  close: (file: LocalAsrTextFile): void => fs.closeSync(file.fd),
  decode: (bytes: Uint8Array): string => util.TextDecoder.create('utf-8').decodeToString(bytes),
};

function readText(path: string, maximumBytes: number): string {
  return LocalAsrTextReader.read(path, maximumBytes, textReaderApi, fs.OpenMode.READ_ONLY);
}

function lookup(path: string, follow: boolean): PathTrustStat | null {
  try {
    const stat: fs.Stat = follow ? fs.statSync(path) : fs.lstatSync(path);
    const found: PathTrustStat = {
      isSymbolicLink: stat.isSymbolicLink(),
      isDirectory: stat.isDirectory(),
      isFile: stat.isFile(),
      uid: stat.uid,
      mode: stat.mode
    };
    return found;
  } catch (error) {
    return null;
  }
}

function lstatPath(path: string): PathTrustStat | null {
  return lookup(path, false);
}

function statPath(path: string): PathTrustStat | null {
  return lookup(path, true);
}

/** 从 `/` 起逐级检查，规则见 `LocalAsrPathTrust`（与 `crates/path-trust/src/lib.rs` 一致）：至多一个仅 root 可改的系统链接，最后一级不能是链接。 */
function trustedDirectory(path: string): boolean {
  return LocalAsrPathTrust.trusted(path, false, lstatPath, statPath);
}

function realFile(path: string): boolean {
  return LocalAsrPathTrust.trusted(path, true, lstatPath, statPath);
}

function readPlan(directory: string, root: string): LocalModelPlan {
  const manifest: string = `${directory}/${LOCAL_MODEL_MANIFEST}`;
  if (LocalAsrPolicy.modelUnderRoot(directory, root).length === 0
      || !trustedDirectory(directory) || !realFile(manifest)) {
    throw new Error('所选路径不是已安装的本地语音模型');
  }
  const plan: LocalModelPlan | null = LocalAsrPolicy.plan(
    directory,
    readText(manifest, LocalAsrPolicy.textFileLimit('manifest')),
  );
  if (plan === null) throw new Error('本地语音模型清单无效或不受支持');
  const files: string[] = LocalAsrPolicy.requiredFiles(plan);
  if (LocalAsrPolicy.transducerNativeHotwords(plan)) files.push(plan.bpeVocab);
  for (const path of files) {
    if (!realFile(path)) throw new Error('本地语音模型文件不完整，请重新下载');
  }
  return plan;
}

/** 原生热词串与编码它们要用的建模单位，与 `LocalAsr.cpp` 的 `use_hotword_tokens` 相同：token 表决定汉字按独立的词拼写（`bpe`）还是照清单。 */
class TransducerHotwords {
  text: string = '';
  modelingUnit: string = '';
}

function transducerHotwords(plan: LocalModelPlan, words: string[]): TransducerHotwords {
  const result: TransducerHotwords = new TransducerHotwords();
  result.modelingUnit = plan.modelingUnit;
  if (!LocalAsrPolicy.transducerNativeHotwords(plan)) return result;
  const tokens: Set<string> =
    LocalAsrPolicy.tokenSet(readText(plan.tokens, LocalAsrPolicy.textFileLimit('tokens')));
  const spaced: boolean = LocalAsrPolicy.cjkSpelledAsWords(tokens);
  result.modelingUnit = LocalAsrPolicy.hotwordModelingUnit(plan, spaced);
  result.text = words.length === 0 ? '' : LocalAsrPolicy.transducerHotwords(words, tokens, spaced);
  return result;
}

function createOnline(plan: LocalModelPlan, hotwords: string, modelingUnit: string, threads: number): OnlineRecognizer {
  const config: OnlineRecognizerConfig = new OnlineRecognizerConfig();
  config.featConfig.sampleRate = LOCAL_ASR_SAMPLE_RATE;
  config.featConfig.featureDim = 80;
  config.modelConfig.transducer.encoder = plan.encoder;
  config.modelConfig.transducer.decoder = plan.decoder;
  config.modelConfig.transducer.joiner = plan.joiner;
  config.modelConfig.tokens = plan.tokens;
  config.modelConfig.numThreads = threads;
  config.modelConfig.provider = 'cpu';
  if (plan.modelType.length > 0) config.modelConfig.modelType = plan.modelType;
  const native: boolean = LocalAsrPolicy.transducerNativeHotwords(plan);
  if (native) {
    config.modelConfig.modelingUnit = modelingUnit;
    config.modelConfig.bpeVocab = plan.bpeVocab;
  }
  // The HAR's streaming binding has no per-stream hotwords, so they are baked into the recognizer and a dictionary edit costs one reload. Hotwords are only honoured by modified beam search.
  config.decodingMethod = native ? 'modified_beam_search' : 'greedy_search';
  config.maxActivePaths = 4;
  config.hotwordsScore = TRANSDUCER_HOTWORDS_SCORE;
  if (hotwords.length > 0) {
    config.hotwordsBuf = hotwords;
    // The C API reads hotwords_buf_size bytes, so this is the UTF-8 length rather than the UTF-16 one.
    config.hotwordsBufSize = utf8Length(hotwords);
  }
  config.enableEndpoint = true;
  config.rule1MinTrailingSilence = 2.4;
  config.rule2MinTrailingSilence = 1.0;
  config.rule3MinUtteranceLength = 20;
  return new OnlineRecognizer(config);
}

function removeHotwordsFile(path: string): void {
  try {
    if (fs.accessSync(path)) fs.unlinkSync(path);
  } catch (error) {
    KeyboardLog.warn('local ASR hotwords file was not removed');
  }
}

/** 热词写进临时文件后的路径；没有热词、没有临时目录或写不进去时为空串，此时不带热词加载——热词出了问题只少了偏向，不能让这次听写失败。 */
function writeHotwordsFile(hotwords: string, scratch: string): string {
  const path: string = hotwords.length > 0 ? LocalAsrPolicy.hotwordsFile(scratch) : '';
  if (path.length === 0) return '';
  try {
    const file: fs.File = fs.openSync(path,
      fs.OpenMode.CREATE | fs.OpenMode.TRUNC | fs.OpenMode.WRITE_ONLY | fs.OpenMode.NOFOLLOW);
    try {
      fs.writeSync(file.fd, hotwords);
    } finally {
      fs.closeSync(file);
    }
    return path;
  } catch (error) {
    KeyboardLog.warn('local ASR hotwords file could not be written; loading without hotwords');
    removeHotwordsFile(path);
    return '';
  }
}

/** 把热词写进临时文件，交给 `create` 加载识别器；sherpa 在加载时就把文件读完，所以加载完无论成败都删掉它。 */
function withHotwordsFile(hotwords: string, scratch: string,
  create: (path: string) => OfflineRecognizer): OfflineRecognizer {
  const path: string = writeHotwordsFile(hotwords, scratch);
  if (path.length === 0) return create('');
  try {
    return create(path);
  } finally {
    removeHotwordsFile(path);
  }
}

function createOffline(plan: LocalModelPlan, language: string, hotwords: string, modelingUnit: string,
  threads: number, hotwordsFile: string): OfflineRecognizer {
  const config: OfflineHotwordsRecognizerConfig = new OfflineHotwordsRecognizerConfig();
  config.featConfig.sampleRate = LOCAL_ASR_SAMPLE_RATE;
  config.featConfig.featureDim = 80;
  config.modelConfig.numThreads = threads;
  config.modelConfig.provider = 'cpu';
  config.decodingMethod = 'greedy_search';
  if (plan.modelType.length > 0) config.modelConfig.modelType = plan.modelType;
  if (plan.kind === LOCAL_MODEL_OFFLINE_TRANSDUCER) {
    config.modelConfig.transducer.encoder = plan.encoder;
    config.modelConfig.transducer.decoder = plan.decoder;
    config.modelConfig.transducer.joiner = plan.joiner;
    config.modelConfig.tokens = plan.tokens;
    if (LocalAsrPolicy.transducerNativeHotwords(plan)) {
      config.modelConfig.modelingUnit = modelingUnit;
      config.modelConfig.bpeVocab = plan.bpeVocab;
    }
    // 与 `LocalAsr.cpp` 相同：不论有没有热词都用宽 beam，整段解码时 encoder 只跑一遍，beam 的代价只落在 decoder/joiner 上。
    config.decodingMethod = 'modified_beam_search';
    config.maxActivePaths = OFFLINE_TRANSDUCER_MAX_ACTIVE_PATHS;
    config.hotwordsScore = TRANSDUCER_HOTWORDS_SCORE;
    config.hotwordsFile = hotwordsFile;
  } else if (plan.kind === LOCAL_MODEL_OFFLINE_SENSE_VOICE) {
    config.modelConfig.tokens = plan.tokens;
    config.modelConfig.senseVoice.model = plan.model;
    config.modelConfig.senseVoice.language = language;
    config.modelConfig.senseVoice.useItn = true;
  } else {
    config.modelConfig.funasrNano.encoderAdaptor = plan.encoderAdaptor;
    config.modelConfig.funasrNano.llm = plan.llm;
    config.modelConfig.funasrNano.embedding = plan.embedding;
    config.modelConfig.funasrNano.tokenizer = plan.tokenizer;
    config.modelConfig.funasrNano.itn = 1;
    config.modelConfig.funasrNano.hotwords = hotwords;
  }
  return new OfflineRecognizer(config);
}

/** Reuses the loaded recognizer when everything baked into it matches; otherwise drops it first, because two resident models would double the largest allocation the keyboard makes. */
function acquire(request: LocalAsrRequest): LoadedModel {
  const plan: LocalModelPlan = readPlan(request.directory, request.root);
  const threads: number = LocalAsrPolicy.threads(request.threads);
  let hotwords: string = '';
  let modelingUnit: string = plan.modelingUnit;
  let language: string = '';
  if (plan.kind === LOCAL_MODEL_ONLINE_TRANSDUCER || plan.kind === LOCAL_MODEL_OFFLINE_TRANSDUCER) {
    const transducer: TransducerHotwords = transducerHotwords(plan, request.hotwords);
    hotwords = transducer.text;
    modelingUnit = transducer.modelingUnit;
  } else if (plan.kind === LOCAL_MODEL_OFFLINE_SENSE_VOICE) {
    language = LocalAsrPolicy.senseVoiceLanguage(request.language);
  } else if (plan.kind === LOCAL_MODEL_OFFLINE_FUNASR_NANO) {
    hotwords = LocalAsrPolicy.funAsrHotwords(request.hotwords);
  }
  const key: string = JSON.stringify([plan.directory, plan.kind, language, hotwords, modelingUnit, threads]);
  if (loaded !== undefined && loaded.key === key) return loaded;
  loaded = undefined;
  const model: LoadedModel = new LoadedModel(plan);
  model.key = key;
  if (plan.kind === LOCAL_MODEL_ONLINE_TRANSDUCER) {
    model.online = createOnline(plan, hotwords, modelingUnit, threads);
  } else if (plan.kind === LOCAL_MODEL_OFFLINE_TRANSDUCER) {
    model.offline = withHotwordsFile(hotwords, request.scratch, (path: string): OfflineRecognizer =>
      createOffline(plan, language, hotwords, modelingUnit, threads, path));
  } else {
    model.offline = createOffline(plan, language, hotwords, modelingUnit, threads, '');
  }
  loaded = model;
  return model;
}

function transcript(current: Session): string {
  const parts: string[] = current.segments.slice();
  if (current.current.length > 0) parts.push(current.current);
  return LocalAsrPolicy.tidyTranscript(LocalAsrPolicy.joinSegments(parts));
}

function report(current: Session): void {
  const text: string = transcript(current);
  if (text === current.lastPartial) return;
  current.lastPartial = text;
  reply('partial', current.id, text, false);
}

function decodeOnline(current: Session): void {
  const recognizer: OnlineRecognizer = current.model.online as OnlineRecognizer;
  const stream: OnlineStream = current.onlineStream as OnlineStream;
  while (recognizer.isReady(stream)) recognizer.decode(stream);
  current.current = recognizer.getResult(stream).text;
  if (recognizer.isEndpoint(stream)) {
    if (current.current.length > 0) current.segments.push(current.current);
    current.current = '';
    recognizer.reset(stream);
  }
}

function remember(current: Session, samples: Float32Array): void {
  // 多攒一秒再整块丢掉最旧的，免得每个窗口都搬一遍整段缓冲。
  if (current.historyLength + samples.length > current.history.length) {
    const drop: number = Math.max(0, current.historyLength + samples.length - current.historyLimit);
    current.history.copyWithin(0, drop, current.historyLength);
    current.historyLength -= drop;
    current.historyStart += drop;
  }
  current.history.set(samples, current.historyLength);
  current.historyLength += samples.length;
}

/** VAD 的段首往前补上它漏掉的起音，见 `LocalAsrPolicy.vadSegmentStart`。 */
function paddedSegment(current: Session, segment: SpeechSegment): Float32Array {
  const history: Float32Array = current.history.subarray(0, current.historyLength);
  const begin: number = LocalAsrPolicy.vadSegmentStart(history, current.historyStart, segment.start, current.decodedEnd);
  current.decodedEnd = segment.start + segment.samples.length;
  if (begin >= segment.start) return segment.samples;
  const padded: Float32Array = new Float32Array(segment.start - begin + segment.samples.length);
  padded.set(history.subarray(begin - current.historyStart, segment.start - current.historyStart), 0);
  padded.set(segment.samples, segment.start - begin);
  return padded;
}

function drainVad(current: Session): void {
  const vad: Vad = current.vad as Vad;
  const recognizer: OfflineRecognizer = current.model.offline as OfflineRecognizer;
  while (!vad.isEmpty()) {
    // Copied out of the detector's buffer, because the segment outlives the pop below while it is decoded.
    const segment: SpeechSegment = vad.front(false);
    vad.pop();
    if (segment.samples.length === 0) continue;
    const stream: OfflineStream = recognizer.createStream();
    stream.acceptWaveform({ samples: paddedSegment(current, segment), sampleRate: LOCAL_ASR_SAMPLE_RATE });
    recognizer.decode(stream);
    const text: string = recognizer.getResult(stream).text;
    if (text.length > 0) current.segments.push(text);
  }
}

function start(request: LocalAsrRequest): void {
  session = undefined;
  const model: LoadedModel = acquire(request);
  const current: Session = new Session(request.id, model);
  if (model.online !== undefined) {
    current.onlineStream = model.online.createStream();
  } else {
    // FunASR-nano shares a 512-token context between audio and text; past roughly 25 seconds it returns nothing.
    const maxSpeech: number = model.plan.kind === LOCAL_MODEL_OFFLINE_FUNASR_NANO ? 20 : 28;
    // 判停灵敏度只改判定阈值（#10310）：按停顿结束的会话按用户设的灵敏度，其余沿用 0.5，见 `LocalAsrPolicy.vadThreshold`。
    const threshold: number = LocalAsrPolicy.vadThreshold(request.stopSensitivity);
    const silero: SileroVadConfig =
      new SileroVadConfig(model.plan.vad, threshold, 0.25, 0.5, LOCAL_VAD_WINDOW, maxSpeech);
    current.historyLimit = maxSpeech * LOCAL_ASR_SAMPLE_RATE + LOCAL_VAD_HISTORY_SLACK;
    current.history = new Float32Array(current.historyLimit + LOCAL_ASR_SAMPLE_RATE);
    const ten: TenVadConfig = new TenVadConfig('', 0.5, 0.25, 0.5, 256, 20);
    current.vad = new Vad(new VadConfig(silero, ten, LOCAL_ASR_SAMPLE_RATE, false, 1), 60);
  }
  session = current;
  reply('started', request.id, '', false);
}

function accept(request: LocalAsrRequest): void {
  const current: Session | undefined = session;
  if (current === undefined || current.id !== request.id || request.pcm === null) return;
  const samples: Float32Array = LocalAsrPolicy.pcm16ToFloat(request.pcm);
  if (samples.length === 0) return;
  if (current.onlineStream !== undefined) {
    current.onlineStream.acceptWaveform({ samples: samples, sampleRate: LOCAL_ASR_SAMPLE_RATE });
    decodeOnline(current);
  } else {
    // The detector judges each call as a whole, so it is fed one window at a time; a long buffer would merge every pause into one segment.
    const vad: Vad = current.vad as Vad;
    for (let at: number = 0; at < samples.length; at += LOCAL_VAD_WINDOW) {
      const chunk: Float32Array = samples.subarray(at, Math.min(at + LOCAL_VAD_WINDOW, samples.length));
      remember(current, chunk);
      vad.acceptWaveform(chunk);
      drainVad(current);
    }
    // 整段模型只在一段说完后才出文字，主线程按「多久没有新文字」判断说完会在用户连续说话时提前结束录音，所以把 VAD 的说话状态告诉它（`LocalAsrSession::speech_detected`）。只在变化时发送。
    const speaking: boolean = vad.isDetected();
    if (speaking !== current.speaking) {
      current.speaking = speaking;
      reply('speech', current.id, '', false, '', speaking);
    }
  }
  report(current);
}

function finish(request: LocalAsrRequest): void {
  const current: Session | undefined = session;
  if (current === undefined || current.id !== request.id) return;
  session = undefined;
  if (current.onlineStream !== undefined) {
    // 尾部补静音，让最后几个字越过 encoder 的前瞻再结束输入；时长的来由见 `LOCAL_ONLINE_TAIL_SAMPLES`。
    const tail: Float32Array = new Float32Array(LOCAL_ONLINE_TAIL_SAMPLES);
    current.onlineStream.acceptWaveform({ samples: tail, sampleRate: LOCAL_ASR_SAMPLE_RATE });
    current.onlineStream.inputFinished();
    decodeOnline(current);
  } else {
    (current.vad as Vad).flush();
    drainVad(current);
  }
  reply('final', current.id, transcript(current), LocalAsrPolicy.correctsWithPinyin(current.model.plan),
    current.model.plan.kind);
}

function describe(error: Object): string {
  if (error instanceof Error && error.message.length > 0) return error.message;
  return '本地语音识别失败';
}

workerPort.onmessage = (event: MessageEvents): void => {
  const request: LocalAsrRequest = event.data as LocalAsrRequest;
  try {
    if (request.type === 'start') {
      start(request);
    } else if (request.type === 'audio') {
      accept(request);
    } else if (request.type === 'finish') {
      finish(request);
    } else if (request.type === 'cancel') {
      if (session !== undefined && session.id === request.id) session = undefined;
    }
  } catch (error) {
    if (session !== undefined && session.id === request.id) session = undefined;
    reply('error', request.id, describe(error as Object), false);
  }
};

workerPort.onmessageerror = (): void => {
  reply('error', 0, '本地语音识别消息无效', false);
};

workerPort.onerror = (event: ErrorEvent): void => {
  reply('error', 0, event.message.length > 0 ? event.message : '本地语音识别失败', false);
};
