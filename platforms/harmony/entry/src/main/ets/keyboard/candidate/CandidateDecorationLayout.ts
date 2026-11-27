import { CandidateSkinAlign } from "./CandidateSkinCatalogPolicy";

/** Where the mascot is drawn, in the coordinates of the card's parent. */
export interface CandidateDecorationRect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

/**
 * The skin decoration (mascot) over a desktop candidate window, placed by the geometry every host shares; the Windows candidate window (`candidate_decoration_rect` in CandidateSkin.h) is the reference.
 *
 * The window is the package's top inset (the band) taller than the card, and the band is transparent. The mascot is the package's width at the image's own aspect ratio, its bottom `padY` below the card's top edge so it overhangs into the card, aligned along that edge. Artwork taller than the band plus that overlap is scaled down uniformly, keeping its aligned edge and its bottom, rather than squashed into the room.
 */
export class CandidateDecorationLayout {
  /** How far the mascot overhangs into the card: the Windows card's vertical padding (`pad_y`), which Harmony's card has no equivalent of. */
  static readonly OVERLAP_VP: number = 6;

  /** The left edge of a decoration `width` wide over a card spanning [cardLeft, cardRight], kept `padX` in from the side it is aligned to and never left of the window. */
  static left(
    align: CandidateSkinAlign,
    cardLeft: number,
    cardRight: number,
    padX: number,
    width: number,
  ): number {
    const left: number =
      align === "left"
        ? cardLeft + padX
        : align === "center"
          ? (cardLeft + cardRight - width) / 2
          : cardRight - padX - width;
    return Math.max(0, left);
  }

  /** The mascot's rect for an image whose width over height is `aspectRatio`; null when the image or the width has no size. */
  static rect(
    align: CandidateSkinAlign,
    cardLeft: number,
    cardRight: number,
    cardTop: number,
    padX: number,
    padY: number,
    band: number,
    width: number,
    aspectRatio: number,
  ): CandidateDecorationRect | null {
    if (
      !(width > 0) ||
      !(aspectRatio > 0) ||
      !Number.isFinite(width) ||
      !Number.isFinite(aspectRatio)
    ) {
      return null;
    }
    const room: number = Math.max(0, band + padY);
    let drawnWidth: number = width;
    let height: number = width / aspectRatio;
    if (height > room) {
      drawnWidth = width * (room / height);
      height = room;
    }
    if (!(drawnWidth > 0) || !(height > 0)) {
      return null;
    }
    const bottom: number = cardTop + padY;
    return {
      x: CandidateDecorationLayout.left(align, cardLeft, cardRight, padX, drawnWidth),
      y: bottom - height,
      width: drawnWidth,
      height: height,
    };
  }

  /** 电脑只显示候选卡片时，不在它下方铺触屏键盘底色和照片。 */
  static keyboardBackdropVisible(desktop: boolean, candidatesOnly: boolean): boolean {
    return !desktop || !candidatesOnly;
  }

  /** How far down the panel the keyboard backdrop starts: below the band and the root padding above the card, so the band stays transparent and the card's top edge is the window's visible top. Zero, and the backdrop fills the panel, wherever no decoration is drawn, which includes every phone. */
  static backdropInsetVp(
    desktop: boolean,
    decorated: boolean,
    bandVp: number,
    rootPaddingVp: number,
  ): number {
    if (!desktop || !decorated || !(bandVp > 0)) {
      return 0;
    }
    return bandVp + rootPaddingVp;
  }
}
