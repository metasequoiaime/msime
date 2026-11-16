import {
  ClipboardHistoryCapture,
  ClipboardHistoryCaptureHost,
} from "../entry/src/main/ets/keyboard/clipboard/ClipboardHistoryCapture";

function check(value: boolean, message: string): void {
  if (!value) throw new Error(message);
}

class Deferred<T> {
  readonly promise: Promise<T>;
  resolve!: (value: T) => void;
  constructor() {
    this.promise = new Promise<T>((resolve) => {
      this.resolve = resolve;
    });
  }
}

class Host implements ClipboardHistoryCaptureHost {
  enabled = true;
  permission: Promise<boolean> = Promise.resolve(true);
  listener: (() => void) | undefined;
  reads: Deferred<string>[] = [];
  saved: string[] = [];
  unavailable = false;
  watches = 0;
  unwatches = 0;
  allowed(): boolean {
    return this.enabled;
  }
  permitted(): Promise<boolean> {
    return this.permission;
  }
  watch(listener: () => void): void {
    this.watches++;
    this.listener = listener;
  }
  unwatch(listener: () => void): void {
    check(this.listener === listener, "注销同一个系统回调");
    this.unwatches++;
    this.listener = undefined;
  }
  read(): Promise<string> {
    const read = new Deferred<string>();
    this.reads.push(read);
    return read.promise;
  }
  save(text: string): boolean {
    if (this.unavailable) return false;
    this.saved.push(text);
    return true;
  }
  copy(): void {
    this.listener?.();
  }
}

async function settled(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

async function main(): Promise<void> {
  const host = new Host();
  const capture = new ClipboardHistoryCapture(host);
  await capture.start();
  await capture.start();
  check(host.watches === 1 && host.reads.length === 0, "重复启动不订阅两次，也不导入旧正文");
  host.unavailable = true;
  for (let i = 0; i < 3; i++) host.copy();
  check(host.reads.length === 3, "每次通知立即发起独立读取");
  host.reads[2].resolve("Synthetic three");
  host.reads[0].resolve("Synthetic one");
  host.reads[1].resolve("Synthetic two");
  await settled();
  check(host.saved.length === 0, "落盘失败仍保留原文");
  host.unavailable = false;
  await new Promise<void>((resolve) => setTimeout(resolve, 150));
  check(
    host.saved.join("|") === "Synthetic one|Synthetic two|Synthetic three",
    "异步完成顺序不改变复制顺序",
  );
  check(host.reads.length === 3, "重试不改读系统最后一条");
  capture.stop();
  check(host.unwatches === 1, "停止注销系统回调");

  const denied = new Host();
  denied.permission = Promise.resolve(false);
  const deniedCapture = new ClipboardHistoryCapture(denied);
  await deniedCapture.start();
  check(denied.watches === 0 && denied.reads.length === 0, "权限拒绝不订阅也不读取");
  const permission = new Deferred<boolean>();
  denied.permission = permission.promise;
  const starting = deniedCapture.start();
  deniedCapture.stop();
  permission.resolve(true);
  await starting;
  check(denied.watches === 0, "停止后迟到的授权结果不能重新订阅");

  const privacy = new Host();
  const privateCapture = new ClipboardHistoryCapture(privacy);
  await privateCapture.start();
  privacy.copy();
  privacy.enabled = false;
  privateCapture.discardIfDisallowed();
  privacy.copy();
  check(privacy.reads.length === 1, "隐私状态不读取");
  privacy.enabled = true;
  privacy.reads[0].resolve("Synthetic stale private text");
  await settled();
  check(privacy.saved.length === 0, "退出隐私状态不能恢复被丢弃的待保存内容");
  privacy.copy();
  privateCapture.stop();
  privacy.reads[1].resolve("Synthetic after stop");
  await settled();
  check(privacy.saved.length === 0, "隐藏后迟到正文不能保存");

  const bounded = new Host();
  bounded.unavailable = true;
  const boundedCapture = new ClipboardHistoryCapture(bounded);
  await boundedCapture.start();
  for (let i = 0; i < 130; i++) {
    bounded.copy();
    bounded.reads[i].resolve(`Synthetic bounded ${i}`);
    await settled();
  }
  bounded.unavailable = false;
  await new Promise<void>((resolve) => setTimeout(resolve, 150));
  check(bounded.saved.length === ClipboardHistoryCapture.CAPACITY, "队列不超过上限");
  check(
    bounded.saved[0] === "Synthetic bounded 2" && bounded.saved[127] === "Synthetic bounded 129",
    "队列满时保留较新的原文",
  );
  boundedCapture.stop();
  console.log(
    "Harmony clipboard capture: continuous reads, retry, order, permission, privacy, stop and bounds passed",
  );
}

main().catch((error: Error): void => {
  throw error;
});
