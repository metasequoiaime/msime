/**
 * 本包所属的产品版本（edition），对应 Android 的 `AppEdition.java` 和 iOS 的 `MSIMEAppEdition`。版本表是 `shared/contracts/editions.json`：每个版本有一组方案和一个默认方案，偏好里的方案本版本没有时退回默认方案。
 *
 * HarmonyOS 目前只有 build-profile.json5 里的 `default` 一个 product，也就是 full：提供全部方案，默认全拼，与引入版本之前相同，所以 `current()` 返回 full。要发别的版本，得为它加一个 product，把版本 id、方案和默认方案写进这个 product 的构建参数，再让 `current()` 从那里读出来，见 platforms/harmony/README.md 的「产品版本」。
 */
export class AppEdition {
  static readonly FULL_ID: string = "full";
  /** full：提供全部方案，默认全拼。 */
  static readonly FULL: AppEdition = new AppEdition(AppEdition.FULL_ID, null, "quanpin");

  readonly id: string;
  /** 本版本提供的方案（版本表里的方案名）；null 表示全部（full）。 */
  readonly inputSchemes: string[] | null;
  /** 本版本的默认方案，也是偏好里的方案本版本没有时的回退值。 */
  readonly defaultScheme: string;

  private constructor(id: string, inputSchemes: string[] | null, defaultScheme: string) {
    this.id = id;
    this.inputSchemes = inputSchemes;
    this.defaultScheme = defaultScheme;
  }

  /** 由版本 id、方案和默认方案组成的版本；full 不论传入什么都是 `FULL`。方案为空或默认方案不在方案里时抛错：一个声明了版本的包不能被当成别的版本运行。 */
  static of(id: string, inputSchemes: string[], defaultScheme: string): AppEdition {
    if (id === AppEdition.FULL_ID) {
      return AppEdition.FULL;
    }
    const schemes: string[] = inputSchemes.filter(
      (scheme: string, index: number): boolean =>
        scheme.length > 0 && inputSchemes.indexOf(scheme) === index,
    );
    if (id.length === 0 || schemes.length === 0 || !schemes.includes(defaultScheme)) {
      throw new Error("Incomplete edition declaration");
    }
    return new AppEdition(id, schemes, defaultScheme);
  }

  /** 本包的版本。 */
  static current(): AppEdition {
    return AppEdition.FULL;
  }

  isFull(): boolean {
    return this.inputSchemes === null;
  }

  /** 本版本是否提供这个方案（`quanpin`、`wubi` 等偏好取值）。 */
  offers(scheme: string): boolean {
    return this.inputSchemes === null || this.inputSchemes.includes(scheme);
  }

  /** 本版本是否有不止一个方案可选；只有一个方案时没有「选方案」这回事，账号里的方案也不随它同步。 */
  offersSchemeChoice(): boolean {
    return this.inputSchemes === null || this.inputSchemes.length > 1;
  }
}
