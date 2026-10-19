import {
  CandidateApng as Apng,
  CandidateApngPart,
  CandidateApngCompositor,
} from "../entry/src/main/ets/keyboard/candidate/CandidateApng";

let assertions: number = 0;
function check(value: boolean, message: string): void {
  assertions++;
  if (!value) throw new Error(message);
}
function equal(actual: Uint8Array | number[], expected: number[], message: string): void {
  check(JSON.stringify(Array.from(actual)) === JSON.stringify(expected), message);
}
function rejected(bytes: Uint8Array, message: string, remaining: number = 8000000): void {
  let failed: boolean = false;
  try {
    Apng.parse(bytes, remaining);
  } catch (_) {
    failed = true;
  }
  check(failed, message);
}
function join(parts: Uint8Array[]): Uint8Array {
  const result: Uint8Array = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0));
  let offset: number = 0;
  for (const part of parts) {
    result.set(part, offset);
    offset += part.length;
  }
  return result;
}
function u32(value: number): Uint8Array {
  const result: Uint8Array = new Uint8Array(4);
  new DataView(result.buffer).setUint32(0, value);
  return result;
}
const signature: Uint8Array = new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]);
const end: Uint8Array = Apng.chunk("IEND", new Uint8Array(0));
const payload: Uint8Array = new Uint8Array([120, 1, 0]);
function header(width: number, height: number): Uint8Array {
  return Apng.chunk("IHDR", join([u32(width), u32(height), new Uint8Array([8, 6, 0, 0, 0])]));
}
function control(count: number, loops: number): Uint8Array {
  return Apng.chunk("acTL", join([u32(count), u32(loops)]));
}
function frame(
  sequence: number,
  width: number = 2,
  height: number = 1,
  x: number = 0,
  y: number = 0,
  dispose: number = 0,
  blend: number = 0,
  numerator: number = 5,
  denominator: number = 100,
): Uint8Array {
  const data: Uint8Array = join([
    u32(sequence),
    u32(width),
    u32(height),
    u32(x),
    u32(y),
    new Uint8Array(6),
  ]);
  const fields: DataView = new DataView(data.buffer);
  fields.setUint16(20, numerator);
  fields.setUint16(22, denominator);
  data[24] = dispose;
  data[25] = blend;
  return Apng.chunk("fcTL", data);
}
function data(sequence: number): Uint8Array {
  return Apng.chunk("fdAT", join([u32(sequence), payload]));
}
function animation(chunks: Uint8Array[], width: number = 2, height: number = 1): Uint8Array {
  return join([signature, header(width, height), ...chunks, end]);
}
const firstData: Uint8Array = Apng.chunk("IDAT", payload);
const common: Uint8Array[] = [control(2, 3), frame(0), firstData, frame(1, 1, 1, 1), data(2)];
const bytes: Uint8Array = animation(common);
const parsed: Apng | null = Apng.parse(bytes, 8000000);
check(parsed !== null && parsed.parts.length === 2 && parsed.loops === 3, "读取帧数和总循环数");
const windowed: Uint8Array = join([new Uint8Array(7), bytes, new Uint8Array(3)]);
check(
  Apng.parse(windowed.subarray(7, 7 + bytes.length), 8000000)?.parts.length === 2,
  "解析 Uint8Array 视图时遵守 byteOffset 和 byteLength",
);
check(parsed !== null && parsed.parts[1].x === 1 && parsed.parts[1].width === 1, "读取子帧偏移");
check(parsed !== null && parsed.parts[0].delay === 50, "读取分数帧时长");
check(
  Apng.crc(new Uint8Array([49, 50, 51, 52, 53, 54, 55, 56, 57]), 0, 9) === 0xcbf43926,
  "CRC32 标准向量",
);
check(Apng.parse(animation([firstData]), 8000000) === null, "静态 PNG 留给系统解码");
check(
  Apng.parse(animation([Apng.chunk("tEXt", new Uint8Array(1048577)), firstData]), 8000000) === null,
  "超过 1MiB 的合法静态资源仍使用共享 8MiB 上限",
);
check(Apng.parse(new Uint8Array([71, 73, 70, 56]), 8000000) === null, "其他图片容器留给系统解码");
check(
  Apng.parse(animation([control(1, 0), firstData, frame(0, 1, 1, 1), data(1)]), 8000000)?.parts[0]
    .usesDefault === false,
  "默认图排除在动画外，动画第一帧可以是子矩形",
);
check(
  Apng.parse(animation([control(1, 0), frame(0, 2, 1, 0, 0, 0, 0, 1, 0), firstData]), 8000000)
    ?.parts[0].delay === 100,
  "分母零按百分秒，超短帧归一化为 100ms",
);
const corrupted: Uint8Array = bytes.slice();
corrupted[60] ^= 1;
rejected(corrupted, "拒绝 CRC 错误");
rejected(bytes.subarray(0, bytes.length - 1), "拒绝截断动画");
rejected(join([bytes, new Uint8Array([0])]), "拒绝结束块后的多余数据");
rejected(bytes, "在解码前检查资源剩余预算", 3);
rejected(animation([control(121, 0), frame(0), firstData]), "拒绝超帧数");
rejected(animation([control(1, 0), frame(0), firstData], 2049, 1), "拒绝超边长");
rejected(animation([control(2, 0), frame(0), firstData], 2048, 2048), "拒绝累计像素超预算");
rejected(animation([control(1, 0), frame(0), firstData, frame(1), data(2)]), "拒绝比声明多的帧");
rejected(animation([control(2, 0), frame(0), firstData]), "拒绝比声明少的帧");
rejected(animation([control(1, 0), control(1, 0), frame(0), firstData]), "拒绝重复动画声明");
rejected(animation([frame(0), control(1, 0), firstData]), "拒绝在动画声明前出现帧控制");
rejected(animation([firstData, control(1, 0), frame(0), data(1)]), "拒绝在默认图数据后声明动画");
rejected(animation([control(1, 0), frame(1), firstData]), "帧控制必须从序号零开始");
rejected(
  animation([control(2, 0), frame(0), firstData, frame(1), data(3)]),
  "拒绝帧数据序号不连续",
);
rejected(animation([control(2, 0), frame(0), frame(1), firstData]), "拒绝无数据帧");
rejected(animation([control(1, 0), frame(0, 1), firstData]), "默认图作为动画帧时必须覆盖整张图");
rejected(
  animation([control(2, 0), frame(0), firstData, frame(1, 2, 1, 1), data(2)]),
  "拒绝子帧出界",
);
rejected(animation([control(1, 0), frame(0, 2, 1, 0, 0, 3), firstData]), "拒绝未知 disposal");
rejected(animation([control(1, 0), frame(0, 2, 1, 0, 0, 0, 2), firstData]), "拒绝未知 blend");
rejected(
  animation([control(2, 0), frame(0), firstData, data(1), frame(2), data(3)]),
  "默认图不能混入 fdAT",
);
rejected(
  animation([control(2, 0), frame(0), firstData, frame(1), firstData]),
  "动画子帧不能使用 IDAT",
);
rejected(
  animation([control(1, 0), frame(0), firstData, Apng.chunk("tEXt", payload), firstData]),
  "默认图 IDAT 必须连续",
);
rejected(
  animation([control(1, 0), frame(0), firstData, Apng.chunk("ABCD", payload)]),
  "拒绝未知关键块",
);
rejected(join([signature, new Uint8Array(8 * 1024 * 1024)]), "限制压缩输入大小");
rejected(
  animation([control(1, 0), frame(0), Apng.chunk("IDAT", new Uint8Array([0, 156, 0]))]),
  "拒绝系统解码器可能容忍的损坏 zlib 头",
);
rejected(
  animation([control(1, 0), frame(0), Apng.chunk("IDAT", new Uint8Array([120, 0, 0]))]),
  "拒绝错误的 zlib FCHECK",
);
rejected(
  animation([control(1, 0), frame(0), Apng.chunk("IDAT", new Uint8Array([120, 32, 0]))]),
  "PNG 帧不允许 zlib 预设字典",
);
check(
  Apng.parse(
    animation([
      control(1, 0),
      frame(0),
      Apng.chunk("IDAT", payload.subarray(0, 1)),
      Apng.chunk("IDAT", payload.subarray(1)),
    ]),
    8000000,
  ) !== null,
  "zlib 头可以跨 IDAT 分块",
);
const palette: Uint8Array = Apng.chunk("PLTE", new Uint8Array([255, 0, 0]));
const transparency: Uint8Array = Apng.chunk("tRNS", new Uint8Array([128]));
const inherited: Apng | null = Apng.parse(
  animation([
    control(2, 0),
    palette,
    transparency,
    frame(0),
    firstData,
    frame(1, 1, 1, 1),
    data(2),
  ]),
  8000000,
);
check(inherited !== null, "允许子帧继承调色板和透明度");
if (inherited !== null) {
  const png: Uint8Array = inherited.png(inherited.parts[1]);
  const view: DataView = new DataView(png.buffer);
  check(view.getUint32(16) === 1 && view.getUint32(20) === 1, "重建 PNG 使用子帧尺寸");
  equal(png.subarray(33, 33 + palette.length), Array.from(palette), "重建保留调色板");
  equal(
    png.subarray(33 + palette.length, 33 + palette.length + transparency.length),
    Array.from(transparency),
    "重建保留透明度",
  );
  check(Apng.parse(png, 8000000) === null, "重建子帧是无动画控制块的静态 PNG");
}

function part(
  x: number = 0,
  dispose: number = 0,
  blend: number = 0,
  width: number = 1,
): CandidateApngPart {
  const result: CandidateApngPart = new CandidateApngPart();
  result.width = width;
  result.height = 1;
  result.x = x;
  result.dispose = dispose;
  result.blend = blend;
  return result;
}
const nativePixels: Uint8Array = new Uint8Array([48, 32, 240, 255, 32, 220, 240, 128]);
CandidateApngCompositor.rgbaFromBgra(nativePixels, false);
equal(
  nativePixels,
  [240, 32, 48, 255, 240, 220, 32, 128],
  "Image Kit 区域读取的 BGRA 转为 RGBA，保留透明度",
);
CandidateApngCompositor.rgbaFromBgra(nativePixels, true);
equal(nativePixels, [48, 32, 240, 255, 32, 220, 240, 255], "无 alpha 原生像素统一为完全不透明");
const red: Uint8Array = new Uint8Array([255, 0, 0, 255]);
const green: Uint8Array = new Uint8Array([0, 255, 0, 255]);
const blue: Uint8Array = new Uint8Array([0, 0, 255, 255]);
const transparent: Uint8Array = new Uint8Array([255, 255, 255, 0]);
const compositor: CandidateApngCompositor = new CandidateApngCompositor(2, 1);
const snapshot: Uint8Array = compositor.render(part(), red, false);
equal(snapshot, [255, 0, 0, 255, 0, 0, 0, 0], "首帧从透明缓冲开始");
equal(
  compositor.render(part(1), blue, false),
  [255, 0, 0, 255, 0, 0, 255, 255],
  "偏移子帧保留其他区域",
);
equal(snapshot, [255, 0, 0, 255, 0, 0, 0, 0], "旧帧快照不被后帧修改");
equal(
  compositor.render(part(1), transparent, false),
  [255, 0, 0, 255, 0, 0, 0, 0],
  "SOURCE 透明像素清除旧内容",
);
equal(
  compositor.render(part(0, 0, 1), new Uint8Array([0, 0, 255, 128]), false),
  [127, 0, 128, 255, 0, 0, 0, 0],
  "OVER 直通 alpha 正确预乘和叠加",
);
equal(
  compositor.render(part(0, 0, 1), transparent, false),
  [127, 0, 128, 255, 0, 0, 0, 0],
  "OVER 全透明保留背景",
);
const background: CandidateApngCompositor = new CandidateApngCompositor(2, 1);
background.render(part(0, 1), red, false);
equal(
  background.render(part(1), green, false),
  [0, 0, 0, 0, 0, 255, 0, 255],
  "BACKGROUND 在下一帧前清除子区域",
);
const previous: CandidateApngCompositor = new CandidateApngCompositor(2, 1);
previous.render(part(0), red, false);
previous.render(part(0, 2), blue, false);
equal(
  previous.render(part(1), green, false),
  [255, 0, 0, 255, 0, 255, 0, 255],
  "PREVIOUS 恢复绘制前的画布",
);
const firstPrevious: CandidateApngCompositor = new CandidateApngCompositor(2, 1);
firstPrevious.render(part(0, 2), red, false);
equal(
  firstPrevious.render(part(1), green, false),
  [0, 0, 0, 0, 0, 255, 0, 255],
  "首帧 PREVIOUS 当作 BACKGROUND",
);
const premul: CandidateApngCompositor = new CandidateApngCompositor(1, 1);
equal(
  premul.render(part(), new Uint8Array([128, 0, 0, 128]), true),
  [128, 0, 0, 128],
  "预乘源不能重复乘 alpha",
);
equal(
  premul.render(part(0, 0, 1), new Uint8Array([0, 0, 128, 128]), true),
  [64, 0, 128, 192],
  "两个半透明预乘像素 OVER",
);
let badRgba: boolean = false;
try {
  premul.render(part(), new Uint8Array(3), true);
} catch (_) {
  badRgba = true;
}
check(badRgba, "拒绝尺寸不匹配的原生像素");
console.log(`candidate-apng: ${assertions} assertions passed`);
