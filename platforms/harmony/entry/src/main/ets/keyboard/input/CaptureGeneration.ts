/** 音频采集的启动代数；停止请求可以在异步启动的任意等待点使旧启动失效。 */
export class CaptureGeneration {
  private generation: number = 0;

  /** 先声明一次启动，再等待释放旧采集器。 */
  begin(): number {
    this.generation += 1;
    return this.generation;
  }

  /** 使当前启动和已经创建的采集器都失效。 */
  invalidate(): void {
    this.generation += 1;
  }

  /** 判断异步启动是否仍属于当前请求。 */
  isCurrent(generation: number): boolean {
    return generation === this.generation;
  }
}
