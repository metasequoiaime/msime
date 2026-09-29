/**
 * Bounded space-drag distance accumulator tied to one editor connection, ported from
 * platforms/android/java/app/msime/android/SpaceCursorMovement.java.
 *
 * The document identity matters: a drag that started against one editor must not keep moving a
 * cursor in another, so every advance re-checks it. Anything implausible cancels rather than
 * clamping, because a jump that large means the gesture was already lost.
 */
const MAXIMUM_JUMP: number = 4096;

export class SpaceCursorMovement {
  private previous: number = 0;
  private remainder: number = 0;
  private document: object | null = null;

  isActive(): boolean {
    return this.document !== null;
  }

  begin(position: number, document: object | null): void {
    if (!Number.isFinite(position) || document === null) {
      this.cancel();
      return;
    }
    this.document = document;
    this.previous = position;
    this.remainder = 0;
  }

  /** Whole steps to move, with the leftover distance carried into the next call. */
  advance(position: number, document: object | null, pixelsPerStep: number): number {
    if (
      !this.isActive() ||
      this.document !== document ||
      !Number.isFinite(position) ||
      !Number.isFinite(pixelsPerStep) ||
      pixelsPerStep < 1
    ) {
      this.cancel();
      return 0;
    }
    const change: number = position - this.previous;
    if (Math.abs(change) > MAXIMUM_JUMP) {
      this.cancel();
      return 0;
    }
    this.remainder += change;
    this.previous = position;
    const steps: number = Math.trunc(this.remainder / pixelsPerStep);
    this.remainder -= steps * pixelsPerStep;
    return steps;
  }

  cancel(): void {
    this.document = null;
    this.previous = 0;
    this.remainder = 0;
  }
}
