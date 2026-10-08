/**
 * 键盘初始自带的常用语，以及添加它们的一次性规则，仿照 Android 的 `CommonPhrasesStore.load`。
 *
 * 由有史以来第一次加载决定：列表为空（没有常用语也没有常用语包）时逐条添加初始常用语，无论是否添加，都会在 `CommonPhrases.json` 旁写一个标记文件，此后不再做这个决定。因此用户清空的列表保持为空，用户删掉的初始常用语也不会回来。标记文件按行记录实际添加的初始常用语，与 Android 一致。
 */
export class CommonPhraseStarters {
  /** 标记文件，与 `CommonPhrases.json` 在同一个状态目录下。 */
  static readonly MARKER_FILE_NAME: string = "common-phrases-seeded";

  /** 按 Android `STARTER_PHRASES` 的顺序：即设计里的列表，只是其中的示例邮箱一行（那会是别人的地址）换成了「稍等，我马上回来」。每条都不含换行，所以标记文件可以一行一条。 */
  static readonly TEXTS: string[] = [
    "好的，收到",
    "我在开会，稍后回复你",
    "马上到",
    "辛苦了，谢谢！",
    "稍等，我马上回来",
    "方便的时候回个电话",
    "周末一起吃饭吗？",
    "已处理，请查收",
  ];

  /** 本次加载是否应添加初始常用语：只在标记文件不存在时，且只对既没有常用语也没有常用语包的列表添加。 */
  static seeds(markerExists: boolean, phraseCount: number, packCount: number): boolean {
    return !markerExists && phraseCount === 0 && packCount === 0;
  }

  /** 标记文件的内容：已添加的初始常用语，一行一条。 */
  static marker(seeded: string[]): string {
    return seeded.join("\n");
  }
}
