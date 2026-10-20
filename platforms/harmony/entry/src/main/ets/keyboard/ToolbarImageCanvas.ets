import { image } from '@kit.ImageKit';
import { ToolbarImageScene, ToolbarImageSlot, ToolbarImagePolicy } from './ToolbarImagePolicy';
import { CandidateCanvasDecoder, CanvasDecodeLease, DecodedCanvasScene,
  DecodedCanvasImage } from './candidate/CandidateCanvasFrame';
import { CandidateCanvasPolicy, CandidateCanvasScene } from './candidate/CandidateCanvasPolicy';
import { FloatingToolbarLayout, ToolbarState } from './FloatingToolbarLayout';
import { ToolbarSkin } from './ToolbarSkinPolicy';
import { KeyboardSession } from './KeyboardSession';

/** 位图与按钮消费同一份共享矩形；窗口形状包含所有动画帧的 alpha 并集。 */
@Component
export struct ToolbarImageCanvas {
  @Prop @Watch('reload') scene: ToolbarImageScene;
  @Prop state: ToolbarState;
  @Prop skin: ToolbarSkin;
  @Prop @Watch('refreshShape') imageScale: number = 1;
  onShape: (key: string, mask: number[][]) => void = (): void => {};
  onFailure: (key: string) => void = (): void => {};
  @State private revision: number = 0;
  private context: CanvasRenderingContext2D = new CanvasRenderingContext2D(new RenderingContextSettings(true));
  private decoded: DecodedCanvasScene | null = null;
  private lease: CanvasDecodeLease | null = null;
  private leaseKey: string = '';
  private alpha: Uint8Array | null = null;
  private generation: number = 0;
  private timer: number = -1;
  private startedAt: number = 0;
  private visible: boolean = true;
  private ready: boolean = false;

  aboutToAppear(): void { this.reload(); }

  aboutToDisappear(): void {
    this.generation++;
    this.stopTimer();
    this.release();
  }

  private release(): void {
    if (this.lease !== null) CandidateCanvasDecoder.release(this.leaseKey, this.lease);
    this.lease = null;
    this.decoded = null;
    this.alpha = null;
    this.leaseKey = '';
  }

  private reload(): void {
    if (this.scene.key === this.leaseKey && this.lease !== null) {
      this.refreshShape();
      this.repaint();
      return;
    }
    const generation: number = ++this.generation;
    this.stopTimer();
    this.release();
    const source: CandidateCanvasScene = { key: this.scene.key, dark: false, resources: this.scene.resources };
    const lease: CanvasDecodeLease = CandidateCanvasDecoder.retain(source);
    this.lease = lease;
    this.leaseKey = source.key;
    void lease.promise.then(async (decoded: DecodedCanvasScene): Promise<void> => {
      if (this.generation !== generation) return;
      this.decoded = decoded;
      const background: DecodedCanvasImage | null = decoded.find(this.scene.spec.image);
      if (background !== null) {
        const alpha: Uint8Array = new Uint8Array(background.width * background.height);
        for (const frame of background.frames) {
          const pixels: ArrayBuffer = new ArrayBuffer(alpha.length * 4);
          await frame.readPixels({ pixels: pixels, offset: 0, stride: background.width * 4,
            region: { x: 0, y: 0, size: { width: background.width, height: background.height } } });
          if (this.generation !== generation) return;
          const info: image.ImageInfo = await frame.getImageInfo();
          const bytes: Uint8Array = new Uint8Array(pixels);
          for (let at: number = 0; at < alpha.length; at++) {
            alpha[at] = info.alphaType === image.AlphaType.OPAQUE ? 255 : Math.max(alpha[at], bytes[at * 4 + 3]);
          }
        }
        this.alpha = alpha;
      }
      if (this.generation !== generation) return;
      this.startedAt = Date.now();
      this.revision++;
      this.refreshShape();
      this.play();
    }).catch((): void => {
      if (this.generation === generation) this.onFailure(this.scene.key);
    });
  }

  private refreshShape(): void {
    if (this.decoded === null) return;
    const bitmap: DecodedCanvasImage | null = this.decoded.find(this.scene.spec.image);
    const mask: number[][] | null = ToolbarImagePolicy.mask(this.scene, this.getUIContext().vp2px(this.imageScale),
      this.alpha, bitmap === null ? 0 : bitmap.width, bitmap === null ? 0 : bitmap.height);
    if (mask === null) this.onFailure(this.scene.key);
    else this.onShape(this.scene.key, mask);
  }

  private stopTimer(): void {
    if (this.timer >= 0) clearTimeout(this.timer);
    this.timer = -1;
  }

  private play(): void {
    this.stopTimer();
    this.repaint();
    if (!this.visible || this.decoded === null) return;
    const elapsed: number = Math.max(0, Date.now() - this.startedAt);
    let next: number = Number.POSITIVE_INFINITY;
    for (const value of this.decoded.images) {
      if (value.frames.length < 2) continue;
      const total: number = value.durations.reduce((sum: number, delay: number): number =>
        sum + CandidateCanvasPolicy.delay(delay), 0);
      if (value.loops > 0 && elapsed >= total * value.loops) continue;
      let boundary: number = -(elapsed % total);
      for (const delay of value.durations) {
        boundary += CandidateCanvasPolicy.delay(delay);
        if (boundary > 0) { next = Math.min(next, boundary); break; }
      }
    }
    if (Number.isFinite(next)) this.timer = setTimeout((): void => this.play(), Math.max(16, next));
  }

  private bitmap(path: string | null | undefined): DecodedCanvasImage | null {
    return path === undefined || path === null || this.decoded === null ? null : this.decoded.find(path);
  }

  private draw(path: string | null | undefined, x: number, y: number, width: number, height: number): void {
    const bitmap: DecodedCanvasImage | null = this.bitmap(path);
    if (bitmap === null) return;
    const elapsed: number = this.visible ? Date.now() - this.startedAt : 0;
    const frame: number = CandidateCanvasPolicy.frame(bitmap.durations, elapsed, bitmap.loops);
    this.context.drawImage(bitmap.frames[frame], x * this.imageScale, y * this.imageScale,
      width * this.imageScale, height * this.imageScale);
  }

  private repaint(): void {
    if (!this.ready) return;
    this.context.clearRect(0, 0, this.scene.layout.widthDip * this.imageScale, this.scene.layout.heightDip * this.imageScale);
    this.draw(this.scene.spec.image, 0, 0, this.scene.spec.imageWidthDip, this.scene.spec.imageHeightDip);
    for (const slot of this.scene.layout.buttons) {
      this.draw(slot.background, slot.xDip, slot.yDip, slot.widthDip, slot.heightDip);
      this.draw(slot.image, slot.xDip, slot.yDip, slot.widthDip, slot.heightDip);
    }
  }

  private face(slot: ToolbarImageSlot): string {
    return this.bitmap(slot.image) !== null ? ''
      : FloatingToolbarLayout.face(ToolbarImagePolicy.button(slot.key), this.state);
  }

  build() {
    Stack({ alignContent: Alignment.TopStart }) {
      Canvas(this.context).width('100%').height('100%')
        .hitTestBehavior(HitTestMode.None)
        .onReady((): void => { this.ready = true; this.repaint(); })
      // 透明孔由窗口形状裁掉；背景内其余位置可以拖动，按钮叠在其上。
      Rect().width(this.scene.spec.imageWidthDip * this.imageScale).height(this.scene.spec.imageHeightDip * this.imageScale)
        .fill('#00000000')
        .gesture(PanGesture({ direction: PanDirection.All, distance: 1 })
          .onActionStart((): void => KeyboardSession.shared.beginToolbarDrag())
          .onActionUpdate((event: GestureEvent): void => KeyboardSession.shared.dragToolbar(event.offsetX, event.offsetY)))
      ForEach(this.scene.layout.buttons, (slot: ToolbarImageSlot) => {
        Button({ type: ButtonType.Normal }) {
          Text(this.face(slot)).fontSize(slot.iconSizeDip * this.imageScale)
            .fontColor(this.skin.buttonColor).fontFamily(this.skin.fontFamily)
        }
        .width(slot.widthDip * this.imageScale).height(slot.heightDip * this.imageScale)
        .position({ x: slot.xDip * this.imageScale, y: slot.yDip * this.imageScale })
        .padding(0).borderRadius(0).backgroundColor('#00000000')
        .accessibilityText(FloatingToolbarLayout.face(ToolbarImagePolicy.button(slot.key), this.state))
        .onClick((): void => KeyboardSession.shared.runToolbarButton(ToolbarImagePolicy.button(slot.key)))
      }, (slot: ToolbarImageSlot): string => `${slot.key}:${this.revision}`)
    }
    .width(this.scene.layout.widthDip * this.imageScale).height(this.scene.layout.heightDip * this.imageScale)
    .onVisibleAreaChange([0, 1], (visible: boolean): void => { this.visible = visible; this.play(); })
  }
}
