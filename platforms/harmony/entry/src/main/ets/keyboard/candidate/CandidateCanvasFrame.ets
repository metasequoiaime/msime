import util from '@ohos.util';
import { image } from '@kit.ImageKit';
import { CandidateApng, CandidateApngCompositor } from './CandidateApng';
import { CandidateCanvasPolicy, CandidateCanvasScene,
  CandidateCanvasDecoration, CandidateCanvasRect, CandidateCanvasGeometry } from './CandidateCanvasPolicy';

export class DecodedCanvasImage {
  relative: string = '';
  width: number = 0;
  height: number = 0;
  frames: image.PixelMap[] = [];
  durations: number[] = [];
  loops: number = 0;
}

export class DecodedCanvasScene {
  images: DecodedCanvasImage[] = [];

  find(relative: string): DecodedCanvasImage | null {
    for (const value of this.images) {
      if (value.relative === relative) return value;
    }
    return null;
  }

  release(): void {
    for (const value of this.images) {
      for (const frame of value.frames) void frame.release();
      value.frames = [];
    }
    this.images = [];
  }
}

export class CanvasDecodeLease {
  readers: number = 0;
  promise: Promise<DecodedCanvasScene>;

  constructor(scene: CandidateCanvasScene) {
    this.promise = CanvasDecodeLease.decode(scene);
  }

  private static async decode(scene: CandidateCanvasScene): Promise<DecodedCanvasScene> {
    const result: DecodedCanvasScene = new DecodedCanvasScene();
    let remaining: number = CandidateCanvasPolicy.MAX_PIXELS;
    for (const resource of scene.resources) {
      let source: image.ImageSource | null = null;
      let frames: image.PixelMap[] = [];
      try {
        const bytes: Uint8Array = new util.Base64Helper().decodeSync(resource.base64);
        const apng: CandidateApng | null = CandidateApng.parse(bytes, remaining);
        if (apng !== null) {
          const decoded: DecodedCanvasImage = await CanvasDecodeLease.decodeApng(apng);
          decoded.relative = resource.relative;
          result.images.push(decoded);
          remaining -= decoded.width * decoded.height * decoded.frames.length;
          continue;
        }
        source = image.createImageSource(bytes.buffer as ArrayBuffer);
        const info: image.ImageInfo = await source.getImageInfo();
        let count: number = 1;
        try { count = await source.getFrameCount(); } catch (_) {
          // 设备不认识动画容器时仍尝试其静态首帧。
        }
        if (!CandidateCanvasPolicy.acceptsImage(info.size.width, info.size.height, count, remaining)) continue;
        try {
          frames = await source.createPixelMapList();
        } catch (_) {
          frames = [await source.createPixelMap()];
        }
        if (frames.length !== count && frames.length !== 1) continue;
        const decoded: DecodedCanvasImage = new DecodedCanvasImage();
        decoded.relative = resource.relative;
        decoded.width = info.size.width;
        decoded.height = info.size.height;
        decoded.frames = frames;
        decoded.loops = CandidateCanvasPolicy.loops(resource.contentType, bytes);
        try {
          decoded.durations = await source.getDelayTimeList();
        } catch (_) {
          decoded.durations = [];
        }
        if (decoded.durations.length !== frames.length) decoded.durations = frames.map((): number => 100);
        result.images.push(decoded);
        remaining -= info.size.width * info.size.height * frames.length;
        frames = [];
      } catch (_) {
        // 单张图失败不影响文字、其他挂件与可读回退；不把资源内容写入日志。
      } finally {
        for (const frame of frames) void frame.release();
        if (source !== null) void source.release();
      }
    }
    return result;
  }

  private static async decodeApng(apng: CandidateApng): Promise<DecodedCanvasImage> {
    const decoded: DecodedCanvasImage = new DecodedCanvasImage();
    decoded.width = apng.width;
    decoded.height = apng.height;
    decoded.loops = apng.loops;
    const compositor: CandidateApngCompositor = new CandidateApngCompositor(apng.width, apng.height);
    try {
      for (const part of apng.parts) {
        const png: Uint8Array = apng.png(part);
        const source: image.ImageSource = image.createImageSource(png.buffer as ArrayBuffer);
        let tile: image.PixelMap | null = null;
        try {
          tile = await source.createPixelMap({ desiredPixelFormat: image.PixelMapFormat.RGBA_8888 });
          const info: image.ImageInfo = await tile.getImageInfo();
          if (info.size.width !== part.width || info.size.height !== part.height
            || info.pixelFormat !== image.PixelMapFormat.RGBA_8888
            || (info.alphaType !== image.AlphaType.PREMUL && info.alphaType !== image.AlphaType.UNPREMUL
              && info.alphaType !== image.AlphaType.OPAQUE)) throw new Error('invalid candidate PNG pixels');
          // readPixels 明确指定紧密行距，不依赖解码器内部 stride 或 DMA 对齐。
          const buffer: ArrayBuffer = new ArrayBuffer(part.width * part.height * 4);
          await tile.readPixels({ pixels: buffer, offset: 0, stride: part.width * 4,
            region: { x: 0, y: 0, size: { width: part.width, height: part.height } } });
          const rgba: Uint8Array = new Uint8Array(buffer);
          // readPixels 固定返回 BGRA，不能从源 PixelMap 的 RGBA 格式推断字节顺序。
          CandidateApngCompositor.rgbaFromBgra(rgba, info.alphaType === image.AlphaType.OPAQUE);
          const pixels: Uint8Array = compositor.render(part, rgba, info.alphaType === image.AlphaType.PREMUL);
          const frame: image.PixelMap = await image.createPixelMap(pixels.buffer as ArrayBuffer,
            { size: { width: apng.width, height: apng.height }, pixelFormat: image.PixelMapFormat.RGBA_8888,
              srcPixelFormat: image.PixelMapFormat.RGBA_8888, alphaType: image.AlphaType.PREMUL });
          decoded.frames.push(frame);
          decoded.durations.push(part.delay);
        } finally {
          if (tile !== null) await tile.release();
          await source.release();
        }
      }
      return decoded;
    } catch (_) {
      for (const frame of decoded.frames) await frame.release();
      throw new Error('candidate APNG decode failed');
    }
  }
}

/** 普通条与展开区共享解码；最后一个读者离开后释放位图和迟到的结果。 */
export class CandidateCanvasDecoder {
  private static leases: Map<string, CanvasDecodeLease> = new Map<string, CanvasDecodeLease>();

  static retain(scene: CandidateCanvasScene): CanvasDecodeLease {
    let lease: CanvasDecodeLease | undefined = CandidateCanvasDecoder.leases.get(scene.key);
    if (lease === undefined) {
      lease = new CanvasDecodeLease(scene);
      CandidateCanvasDecoder.leases.set(scene.key, lease);
    }
    lease.readers++;
    return lease;
  }

  static release(key: string, lease: CanvasDecodeLease): void {
    lease.readers--;
    if (lease.readers > 0) return;
    CandidateCanvasDecoder.leases.delete(key);
    void lease.promise.then((value: DecodedCanvasScene): void => value.release());
  }
}

/** 候选内容前后的位图层不参与命中测试，候选按键仍由原有宿主选择。 */
@Component
export struct CandidateCanvasFrame {
  @Prop @Watch('reload') scene: CandidateCanvasScene | null = null;
  @Prop @Watch('repaint') surfaceOpacity: number = 1;
  @Prop @Watch('repaint') fallbackColor: string = '#FFFFFF';
  @Prop frameHeight: number | undefined = undefined;
  @Prop fixedContent: boolean = false;
  /** The content the host lays out inside the panel, in vp; the insets are scaled down until it fits. 28 is enough for one line of text, but the phone strip lays out a 34vp candidate lane (and a reading line above it when it has something to show), so it passes its own. */
  @Prop @Watch('repaint') minimumContentHeight: number = 28;
  @Prop @Watch('refreshPlayback') playing: boolean = true;
  onSize: (width: number, height: number) => void = (): void => {};
  onPanelReady: (key: string, ready: boolean) => void = (): void => {};
  @BuilderParam content: () => void = this.emptyContent;
  @State private widthVp: number = 0;
  @State private heightVp: number = 0;
  @State private contentInsets: Padding = {};
  private under: CanvasRenderingContext2D = new CanvasRenderingContext2D(new RenderingContextSettings(true));
  private over: CanvasRenderingContext2D = new CanvasRenderingContext2D(new RenderingContextSettings(true));
  private decoded: DecodedCanvasScene | null = null;
  private lease: CanvasDecodeLease | null = null;
  private leaseKey: string = '';
  private generation: number = 0;
  private timer: number = -1;
  private visible: boolean = true;
  private ready: boolean = false;
  private startedAt: number = 0;
  private notified: boolean = false;

  @Builder
  emptyContent() {}

  aboutToAppear(): void { this.reload(); }

  aboutToDisappear(): void {
    this.generation++;
    this.stopTimer();
    this.dropLease();
  }

  private dropLease(): void {
    if (this.lease !== null) CandidateCanvasDecoder.release(this.leaseKey, this.lease);
    this.lease = null;
    this.decoded = null;
    if (this.leaseKey.length > 0) this.onPanelReady(this.leaseKey, false);
    this.leaseKey = '';
    this.notified = false;
  }

  private reload(): void {
    if (this.scene !== null && this.scene.key === this.leaseKey && this.lease !== null) return;
    const generation: number = ++this.generation;
    this.stopTimer();
    this.dropLease();
    this.repaint();
    if (this.scene === null) return;
    const key: string = this.scene.key;
    const lease: CanvasDecodeLease = CandidateCanvasDecoder.retain(this.scene);
    this.leaseKey = key;
    this.lease = lease;
    void lease.promise.then((decoded: DecodedCanvasScene): void => {
      if (generation !== this.generation || this.lease !== lease) return;
      this.decoded = decoded;
      this.startedAt = Date.now();
      this.refreshPlayback();
    });
  }

  private stopTimer(): void {
    if (this.timer >= 0) clearTimeout(this.timer);
    this.timer = -1;
  }

  private refreshPlayback(): void {
    this.stopTimer();
    this.repaint();
    if (!this.visible || !this.playing || this.decoded === null) return;
    let next: number = Number.POSITIVE_INFINITY;
    const elapsed: number = Math.max(0, Date.now() - this.startedAt);
    for (const value of this.decoded.images) {
      if (value.frames.length <= 1) continue;
      const total: number = value.durations.reduce((sum: number, delay: number): number =>
        sum + CandidateCanvasPolicy.delay(delay), 0);
      if (value.loops > 0 && elapsed >= total * value.loops) continue;
      let boundary: number = -(elapsed % total);
      for (const delay of value.durations) {
        boundary += CandidateCanvasPolicy.delay(delay);
        if (boundary > 0) { next = Math.min(next, boundary); break; }
      }
    }
    if (Number.isFinite(next)) {
      this.timer = setTimeout((): void => this.refreshPlayback(), Math.max(10, next));
    }
  }

  private panelImage(): DecodedCanvasImage | null {
    if (this.scene === null || this.scene.borderImage === null || this.scene.borderImage === undefined
      || this.decoded === null) return null;
    const border = this.scene.borderImage;
    return this.decoded.find(!this.scene.dark && border.lightSource ? border.lightSource : border.source);
  }

  private geometry(): CandidateCanvasGeometry | null {
    return this.scene === null ? null
      : CandidateCanvasPolicy.geometry(this.scene, this.widthVp, this.heightVp, undefined,
        this.minimumContentHeight);
  }

  private frame(value: DecodedCanvasImage): image.PixelMap {
    const elapsed: number = this.playing && this.visible ? Date.now() - this.startedAt : 0;
    return value.frames[CandidateCanvasPolicy.frame(value.durations, elapsed, value.loops)];
  }

  private drawDecoration(value: CandidateCanvasDecoration, context: CanvasRenderingContext2D,
    geometry: CandidateCanvasGeometry): void {
    const bitmap: DecodedCanvasImage | null = this.decoded === null ? null : this.decoded.find(value.image);
    if (bitmap === null) return;
    const rect: CandidateCanvasRect = CandidateCanvasPolicy.decorationRect(value,
      bitmap.width, bitmap.height, geometry.card, geometry.scale);
    if (rect.width > 0 && rect.height > 0) {
      context.drawImage(this.frame(bitmap), rect.x, rect.y, rect.width, rect.height);
    }
  }

  private repaint(): void {
    if (!this.ready) return;
    this.under.clearRect(0, 0, this.widthVp, this.heightVp);
    this.over.clearRect(0, 0, this.widthVp, this.heightVp);
    const geometry: CandidateCanvasGeometry | null = this.geometry();
    if (geometry === null || this.scene === null) { this.contentInsets = {}; return; }
    const previous: Padding = this.contentInsets;
    if (previous.top !== geometry.content.top || previous.right !== geometry.content.right
      || previous.bottom !== geometry.content.bottom || previous.left !== geometry.content.left) {
      this.contentInsets = geometry.content;
    }
    this.under.globalAlpha = this.surfaceOpacity;
    this.over.globalAlpha = this.surfaceOpacity;
    const panel: DecodedCanvasImage | null = this.panelImage();
    if (panel === null) {
      this.under.fillStyle = this.fallbackColor;
      this.under.fillRect(geometry.card.x, geometry.card.y, geometry.card.width, geometry.card.height);
    }
    const decorations: CandidateCanvasDecoration[] = CandidateCanvasPolicy.decorations(this.scene.decorations ?? []);
    for (const value of decorations) {
      if (value.zIndex < 1) this.drawDecoration(value, this.under, geometry);
    }
    let panelDrawn: boolean = false;
    if (panel !== null && this.scene.borderImage !== null && this.scene.borderImage !== undefined) {
      for (const patch of CandidateCanvasPolicy.patches(this.scene.borderImage, panel.width, panel.height,
        geometry.card, geometry.scale)) {
        // 本宿主最低 API 21，PixelMap 裁切坐标按 vp 解释，清单切片则是源像素。
        this.under.drawImage(this.frame(panel), px2vp(patch.source.x), px2vp(patch.source.y),
          px2vp(patch.source.width), px2vp(patch.source.height),
          patch.destination.x, patch.destination.y, patch.destination.width, patch.destination.height);
        panelDrawn = true;
      }
    }
    for (const value of decorations) {
      if (value.zIndex >= 1) this.drawDecoration(value, this.over, geometry);
    }
    if (panelDrawn !== this.notified) {
      this.notified = panelDrawn;
      this.onPanelReady(this.scene.key, panelDrawn);
    }
  }

  build() {
    Stack({ alignContent: Alignment.TopStart }) {
      // matchParent 在内容测量后匹配尺寸，不让画布反过来撑高候选卡片。
      Canvas(this.under).width(LayoutPolicy.matchParent).height(LayoutPolicy.matchParent)
        .hitTestBehavior(HitTestMode.None)
        .onReady((): void => { this.ready = true; this.repaint(); })
      Column() { this.content(); }
        .width('100%')
        .height(this.frameHeight !== undefined ? this.frameHeight : this.fixedContent ? '100%' : undefined)
        .padding(this.contentInsets)
      Canvas(this.over).width(LayoutPolicy.matchParent).height(LayoutPolicy.matchParent)
        .hitTestBehavior(HitTestMode.None)
        .onReady((): void => this.repaint())
    }
    .width('100%')
    .height(LayoutPolicy.wrapContent)
    .onAreaChange((_old: Area, now: Area): void => {
      this.widthVp = Number(now.width);
      this.heightVp = Number(now.height);
      this.onSize(this.widthVp, this.heightVp);
      this.repaint();
    })
    .onVisibleAreaChange([0, 1], (visible: boolean): void => {
      this.visible = visible;
      this.refreshPlayback();
    })
  }
}
