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
  LOCAL_MODEL_ONLINE_TRANSDUCER,
  LOCAL_VAD_WINDOW,
  LocalAsrPolicy,
  LocalAsrReply,
  LocalAsrRequest,
  LocalModelPlan
} from '../keyboard/input/LocalAsrPolicy';
import { LocalAsrPathTrust, PathTrustStat } from '../keyboard/input/LocalAsrPathTrust';
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
  segments: string[] = [];
  current: string = '';
  lastPartial: string = '';

  constructor(id: number, model: LoadedModel) {
    this.id = id;
    this.model = model;
  }
}

let loaded: LoadedModel | undefined = undefined;
let session: Session | undefined = undefined;

function reply(type: string, id: number, text: string, correct: boolean): void {
  const message: LocalAsrReply = { type: type, id: id, text: text, correct: correct };
  workerPort.postMessage(message);
}

const TEXT_READ_CHUNK_BYTES: number = 64 * 1024;

/** Read only the bounded text prefix the worker needs, without an unbounded readTextSync allocation. */
function readText(path: string, maximumBytes: number): string {
  let file: fs.File | undefined = undefined;
  try {
    file = fs.openSync(path, fs.OpenMode.READ_ONLY);
    const bytes: Uint8Array = new Uint8Array(maximumBytes + 1);
    let total: number = 0;
    while (total < bytes.length) {
      const size: number = Math.min(TEXT_READ_CHUNK_BYTES, bytes.length - total);
      const chunk: ArrayBuffer = new ArrayBuffer(size);
      const count: number = fs.readSync(file.fd, chunk);
      if (count < 0 || count > size) throw new Error('本地语音模型文本文件读取失败');
      if (count === 0) break;
      bytes.set(new Uint8Array(chunk, 0, count), total);
      total += count;
      if (total > maximumBytes) throw new Error('本地语音模型文本文件过大');
    }
    return util.TextDecoder.create('utf-8').decode(new Uint8Array(bytes.buffer, 0, total));
  } finally {
    if (file !== undefined) fs.closeSync(file);
  }
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

function readPlan(directory: string): LocalModelPlan {
  const manifest: string = `${directory}/${LOCAL_MODEL_MANIFEST}`;
  if (!trustedDirectory(directory) || !realFile(manifest)) {
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

function transducerHotwords(plan: LocalModelPlan, words: string[]): string {
  if (!LocalAsrPolicy.transducerNativeHotwords(plan) || words.length === 0) return '';
  return LocalAsrPolicy.transducerHotwords(
    words,
    LocalAsrPolicy.tokenSet(readText(plan.tokens, LocalAsrPolicy.textFileLimit('tokens'))),
  );
}

function createOnline(plan: LocalModelPlan, hotwords: string, threads: number): OnlineRecognizer {
  const config: OnlineRecognizerConfig = new OnlineRecognizerConfig();
  config.featConfig.sampleRate = LOCAL_ASR_SAMPLE_RATE;
  config.featConfig.featureDim = 80;
  config.modelConfig.transducer.encoder = plan.encoder;
  config.modelConfig.transducer.decoder = plan.decoder;
  config.modelConfig.transducer.joiner = plan.joiner;
  config.modelConfig.tokens = plan.tokens;
  config.modelConfig.numThreads = threads;
  config.modelConfig.provider = 'cpu';
  const native: boolean = LocalAsrPolicy.transducerNativeHotwords(plan);
  if (native) {
    config.modelConfig.modelingUnit = plan.modelingUnit;
    config.modelConfig.bpeVocab = plan.bpeVocab;
  }
  // The HAR's streaming binding has no per-stream hotwords, so they are baked into the recognizer and a dictionary edit costs one reload. Hotwords are only honoured by modified beam search.
  config.decodingMethod = native ? 'modified_beam_search' : 'greedy_search';
  config.maxActivePaths = 4;
  config.hotwordsScore = 2.0;
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

function createOffline(plan: LocalModelPlan, language: string, hotwords: string, threads: number): OfflineRecognizer {
  const config: OfflineRecognizerConfig = new OfflineRecognizerConfig();
  config.featConfig.sampleRate = LOCAL_ASR_SAMPLE_RATE;
  config.featConfig.featureDim = 80;
  config.modelConfig.numThreads = threads;
  config.modelConfig.provider = 'cpu';
  config.decodingMethod = 'greedy_search';
  if (plan.kind === LOCAL_MODEL_OFFLINE_SENSE_VOICE) {
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
  const plan: LocalModelPlan = readPlan(request.directory);
  const threads: number = LocalAsrPolicy.threads(request.threads);
  let hotwords: string = '';
  let language: string = '';
  if (plan.kind === LOCAL_MODEL_ONLINE_TRANSDUCER) {
    hotwords = transducerHotwords(plan, request.hotwords);
  } else if (plan.kind === LOCAL_MODEL_OFFLINE_SENSE_VOICE) {
    language = LocalAsrPolicy.senseVoiceLanguage(request.language);
  } else if (plan.kind === LOCAL_MODEL_OFFLINE_FUNASR_NANO) {
    hotwords = LocalAsrPolicy.funAsrHotwords(request.hotwords);
  }
  const key: string = JSON.stringify([plan.directory, plan.kind, language, hotwords, threads]);
  if (loaded !== undefined && loaded.key === key) return loaded;
  loaded = undefined;
  const model: LoadedModel = new LoadedModel(plan);
  model.key = key;
  if (plan.kind === LOCAL_MODEL_ONLINE_TRANSDUCER) {
    model.online = createOnline(plan, hotwords, threads);
  } else {
    model.offline = createOffline(plan, language, hotwords, threads);
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

function drainVad(current: Session): void {
  const vad: Vad = current.vad as Vad;
  const recognizer: OfflineRecognizer = current.model.offline as OfflineRecognizer;
  while (!vad.isEmpty()) {
    // Copied out of the detector's buffer, because the segment outlives the pop below while it is decoded.
    const segment: SpeechSegment = vad.front(false);
    vad.pop();
    if (segment.samples.length === 0) continue;
    const stream: OfflineStream = recognizer.createStream();
    stream.acceptWaveform({ samples: segment.samples, sampleRate: LOCAL_ASR_SAMPLE_RATE });
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
    const silero: SileroVadConfig = new SileroVadConfig(model.plan.vad, 0.5, 0.25, 0.5, LOCAL_VAD_WINDOW,
      // FunASR-nano shares a 512-token context between audio and text; past roughly 25 seconds it returns nothing.
      model.plan.kind === LOCAL_MODEL_OFFLINE_FUNASR_NANO ? 20 : 28);
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
      vad.acceptWaveform(samples.subarray(at, Math.min(at + LOCAL_VAD_WINDOW, samples.length)));
      drainVad(current);
    }
  }
  report(current);
}

function finish(request: LocalAsrRequest): void {
  const current: Session | undefined = session;
  if (current === undefined || current.id !== request.id) return;
  session = undefined;
  if (current.onlineStream !== undefined) {
    // Trailing silence lets the last words through the encoder's look-ahead before input ends.
    const tail: Float32Array = new Float32Array(Math.floor(LOCAL_ASR_SAMPLE_RATE * 6 / 10));
    current.onlineStream.acceptWaveform({ samples: tail, sampleRate: LOCAL_ASR_SAMPLE_RATE });
    current.onlineStream.inputFinished();
    decodeOnline(current);
  } else {
    (current.vad as Vad).flush();
    drainVad(current);
  }
  reply('final', current.id, transcript(current), LocalAsrPolicy.correctsWithPinyin(current.model.plan));
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
