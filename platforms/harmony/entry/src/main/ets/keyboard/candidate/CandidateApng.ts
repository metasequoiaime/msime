import { CandidateCanvasPolicy } from "./CandidateCanvasPolicy";

export class CandidateApngPart {
  width: number = 0;
  height: number = 0;
  x: number = 0;
  y: number = 0;
  delay: number = 100;
  dispose: number = 0;
  blend: number = 0;
  data: Uint8Array[] = [];
  usesDefault: boolean = false;
}

/** 只拆动画容器；PNG 像素解压仍交给系统的静态图片解码器。 */
export class CandidateApng {
  width: number = 0;
  height: number = 0;
  loops: number = 0;
  parts: CandidateApngPart[] = [];
  private header: Uint8Array = new Uint8Array(0);
  private shared: Uint8Array[] = [];
  private static readonly SIGNATURE: number[] = [137, 80, 78, 71, 13, 10, 26, 10];
  private static readonly CRC_TABLE: Uint32Array = CandidateApng.crcTable();

  private static crcTable(): Uint32Array {
    const table: Uint32Array = new Uint32Array(256);
    for (let index: number = 0; index < 256; index++) {
      let value: number = index;
      for (let bit: number = 0; bit < 8; bit++) {
        value = (value >>> 1) ^ ((value & 1) === 1 ? 0xedb88320 : 0);
      }
      table[index] = value >>> 0;
    }
    return table;
  }

  static crc(bytes: Uint8Array, start: number, end: number): number {
    let value: number = 0xffffffff;
    for (let index: number = start; index < end; index++) {
      value = CandidateApng.CRC_TABLE[(value ^ bytes[index]) & 255] ^ (value >>> 8);
    }
    return (value ^ 0xffffffff) >>> 0;
  }

  static chunk(type: string, data: Uint8Array): Uint8Array {
    const result: Uint8Array = new Uint8Array(data.length + 12);
    const view: DataView = new DataView(result.buffer);
    view.setUint32(0, data.length);
    for (let index: number = 0; index < 4; index++) result[index + 4] = type.charCodeAt(index);
    result.set(data, 8);
    view.setUint32(result.length - 4, CandidateApng.crc(result, 4, result.length - 4));
    return result;
  }

  private static invalid(): Error {
    return new Error("invalid candidate APNG");
  }

  static parse(bytes: Uint8Array, remaining: number): CandidateApng | null {
    if (bytes.length < 8) return null;
    for (let index: number = 0; index < 8; index++) {
      if (bytes[index] !== CandidateApng.SIGNATURE[index]) return null;
    }
    if (bytes.length > 8 * 1024 * 1024) throw CandidateApng.invalid();
    const result: CandidateApng = new CandidateApng();
    const view: DataView = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    let position: number = 8;
    let count: number = 0;
    let sequence: number = 0;
    let seenData: boolean = false;
    let dataClosed: boolean = false;
    let ended: boolean = false;
    let current: CandidateApngPart | null = null;
    while (position + 12 <= bytes.length) {
      const length: number = view.getUint32(position);
      if (length > bytes.length - position - 12) throw CandidateApng.invalid();
      const end: number = position + length + 12;
      const type: string = String.fromCharCode(
        bytes[position + 4],
        bytes[position + 5],
        bytes[position + 6],
        bytes[position + 7],
      );
      if (CandidateApng.crc(bytes, position + 4, end - 4) !== view.getUint32(end - 4)) {
        throw CandidateApng.invalid();
      }
      if (position === 8 && type !== "IHDR") throw CandidateApng.invalid();
      const data: Uint8Array = bytes.subarray(position + 8, end - 4);
      const fields: DataView = new DataView(data.buffer, data.byteOffset, data.byteLength);
      if (seenData && type !== "IDAT") dataClosed = true;
      switch (type) {
        case "IHDR":
          if (position !== 8 || length !== 13) throw CandidateApng.invalid();
          result.header = data.slice();
          result.width = fields.getUint32(0);
          result.height = fields.getUint32(4);
          break;
        case "acTL":
          if (seenData || count !== 0 || length !== 8) throw CandidateApng.invalid();
          count = fields.getUint32(0);
          result.loops = fields.getUint32(4);
          if (!CandidateCanvasPolicy.acceptsImage(result.width, result.height, count, remaining)) {
            throw CandidateApng.invalid();
          }
          break;
        case "fcTL": {
          if (count === 0 || length !== 26 || fields.getUint32(0) !== sequence++)
            throw CandidateApng.invalid();
          if (current !== null && !CandidateApng.hasData(current)) throw CandidateApng.invalid();
          current = new CandidateApngPart();
          current.width = fields.getUint32(4);
          current.height = fields.getUint32(8);
          current.x = fields.getUint32(12);
          current.y = fields.getUint32(16);
          const denominator: number = fields.getUint16(22);
          current.delay = CandidateCanvasPolicy.delay(
            Math.round((1000 * fields.getUint16(20)) / (denominator === 0 ? 100 : denominator)),
          );
          current.dispose = data[24];
          current.blend = data[25];
          current.usesDefault = !seenData;
          if (
            current.width === 0 ||
            current.height === 0 ||
            current.x + current.width > result.width ||
            current.y + current.height > result.height ||
            current.dispose > 2 ||
            current.blend > 1 ||
            result.parts.length >= count
          )
            throw CandidateApng.invalid();
          if (
            !seenData &&
            (current.width !== result.width ||
              current.height !== result.height ||
              current.x !== 0 ||
              current.y !== 0)
          )
            throw CandidateApng.invalid();
          result.parts.push(current);
          break;
        }
        case "IDAT":
          if (dataClosed || (current !== null && !current.usesDefault))
            throw CandidateApng.invalid();
          seenData = true;
          if (current !== null) current.data.push(data);
          break;
        case "fdAT":
          if (
            !seenData ||
            current === null ||
            current.usesDefault ||
            length < 4 ||
            fields.getUint32(0) !== sequence++
          )
            throw CandidateApng.invalid();
          current.data.push(data.subarray(4));
          break;
        case "IEND":
          if (length !== 0 || end !== bytes.length) throw CandidateApng.invalid();
          ended = true;
          break;
        default:
          // 保留子帧继承的调色板、透明度与色彩信息；未知关键块不能悄悄丢弃。
          if ((bytes[position + 4] & 32) === 0 && type !== "PLTE") throw CandidateApng.invalid();
          if (!seenData) result.shared.push(bytes.subarray(position, end));
      }
      position = end;
    }
    if (count === 0) return null;
    if (
      !ended ||
      !seenData ||
      result.parts.length !== count ||
      current === null ||
      !CandidateApng.hasData(current)
    )
      throw CandidateApng.invalid();
    for (const part of result.parts) {
      if (!CandidateApng.hasZlibHeader(part)) throw CandidateApng.invalid();
    }
    return result;
  }

  private static hasZlibHeader(part: CandidateApngPart): boolean {
    const header: number[] = [];
    for (const data of part.data) {
      for (const byte of data) {
        header.push(byte);
        if (header.length === 2) {
          // PNG 只允许 deflate、至多 32KiB 窗口且不允许预设字典。
          return (
            (header[0] & 15) === 8 &&
            header[0] >>> 4 <= 7 &&
            (header[1] & 32) === 0 &&
            (header[0] * 256 + header[1]) % 31 === 0
          );
        }
      }
    }
    return false;
  }

  private static hasData(part: CandidateApngPart): boolean {
    return part.data.some((value: Uint8Array): boolean => value.length > 0);
  }

  png(part: CandidateApngPart): Uint8Array {
    const header: Uint8Array = this.header.slice();
    const fields: DataView = new DataView(header.buffer);
    fields.setUint32(0, part.width);
    fields.setUint32(4, part.height);
    const chunks: Uint8Array[] = [
      new Uint8Array(CandidateApng.SIGNATURE),
      CandidateApng.chunk("IHDR", header),
    ];
    for (const shared of this.shared) chunks.push(shared);
    for (const data of part.data) chunks.push(CandidateApng.chunk("IDAT", data));
    chunks.push(CandidateApng.chunk("IEND", new Uint8Array(0)));
    const result: Uint8Array = new Uint8Array(
      chunks.reduce((sum: number, value: Uint8Array): number => sum + value.length, 0),
    );
    let offset: number = 0;
    for (const chunk of chunks) {
      result.set(chunk, offset);
      offset += chunk.length;
    }
    return result;
  }
}

/** 工作缓冲和快照均为预乘 RGBA，避免透明边缘被重复乘 alpha。 */
export class CandidateApngCompositor {
  private pixels: Uint8Array;
  private previous: Uint8Array | null = null;
  private last: CandidateApngPart | null = null;
  private index: number = 0;
  private width: number;

  constructor(width: number, height: number) {
    this.width = width;
    this.pixels = new Uint8Array(width * height * 4);
  }

  static rgbaFromBgra(pixels: Uint8Array, opaque: boolean): void {
    for (let index: number = 0; index < pixels.length; index += 4) {
      const blue: number = pixels[index];
      pixels[index] = pixels[index + 2];
      pixels[index + 2] = blue;
      if (opaque) pixels[index + 3] = 255;
    }
  }

  render(part: CandidateApngPart, rgba: Uint8Array, premultiplied: boolean): Uint8Array {
    if (rgba.length !== part.width * part.height * 4) throw new Error("invalid candidate RGBA");
    if (this.last !== null) {
      if (this.last.dispose === 1 || (this.last.dispose === 2 && this.index === 1)) {
        for (let row: number = 0; row < this.last.height; row++) {
          const start: number = ((this.last.y + row) * this.width + this.last.x) * 4;
          this.pixels.fill(0, start, start + this.last.width * 4);
        }
      } else if (this.last.dispose === 2 && this.previous !== null) {
        this.pixels.set(this.previous);
      }
    }
    this.previous = part.dispose === 2 && this.index > 0 ? this.pixels.slice() : null;
    for (let row: number = 0; row < part.height; row++) {
      for (let column: number = 0; column < part.width; column++) {
        const from: number = (row * part.width + column) * 4;
        const to: number = ((row + part.y) * this.width + column + part.x) * 4;
        const alpha: number = rgba[from + 3];
        const behind: number = part.blend === 0 ? 0 : (255 - alpha) / 255;
        const factor: number = premultiplied ? 1 : alpha / 255;
        for (let channel: number = 0; channel < 3; channel++) {
          this.pixels[to + channel] = Math.min(
            255,
            Math.round(rgba[from + channel] * factor + this.pixels[to + channel] * behind),
          );
        }
        this.pixels[to + 3] = Math.round(alpha + this.pixels[to + 3] * behind);
      }
    }
    this.last = part;
    this.index++;
    return this.pixels.slice();
  }
}
