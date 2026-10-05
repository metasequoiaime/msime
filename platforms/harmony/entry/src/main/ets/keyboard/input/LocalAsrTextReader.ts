/** A small file handle shape so the bounded reader can be checked without the Harmony file module. */
export interface LocalAsrTextFile {
  fd: number;
}

export interface LocalAsrTextReaderApi {
  open(path: string, mode: number): LocalAsrTextFile;
  read(fd: number, buffer: ArrayBuffer): number;
  close(file: LocalAsrTextFile): void;
  decode(bytes: Uint8Array): string;
}

const READ_CHUNK_BYTES: number = 64 * 1024;

/** Reads a bounded UTF-8 text file from one open handle, refusing the first byte past the limit. */
export class LocalAsrTextReader {
  static read(
    path: string,
    maximumBytes: number,
    api: LocalAsrTextReaderApi,
    readOnlyMode: number,
  ): string {
    if (!Number.isInteger(maximumBytes) || maximumBytes <= 0) {
      throw new Error("本地语音模型文本文件上限无效");
    }
    let file: LocalAsrTextFile | undefined = undefined;
    try {
      file = api.open(path, readOnlyMode);
      const bytes: Uint8Array = new Uint8Array(maximumBytes + 1);
      let total: number = 0;
      while (total < bytes.length) {
        const size: number = Math.min(READ_CHUNK_BYTES, bytes.length - total);
        const chunk: ArrayBuffer = new ArrayBuffer(size);
        const count: number = api.read(file.fd, chunk);
        if (count < 0 || count > size) throw new Error("本地语音模型文本文件读取失败");
        if (count === 0) break;
        bytes.set(new Uint8Array(chunk, 0, count), total);
        total += count;
        if (total > maximumBytes) throw new Error("本地语音模型文本文件过大");
      }
      return api.decode(new Uint8Array(bytes.buffer, 0, total));
    } finally {
      if (file !== undefined) api.close(file);
    }
  }
}
