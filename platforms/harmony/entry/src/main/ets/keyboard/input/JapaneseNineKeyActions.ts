/**
 * Japanese side-key labels, ported from
 * platforms/android/java/app/msime/client/JapaneseNineKeyActions.java.
 *
 * Composition and conversion stay owned by the Engine; these are only what the two side keys print
 * depending on whether something is being composed.
 */
export class JapaneseNineKeyActions {
  static spaceTitle(composing: boolean): string {
    return composing ? "変換" : "空白";
  }

  static returnTitle(composing: boolean): string {
    return composing ? "確定" : "改行";
  }
}
