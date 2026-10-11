import { utf8Length } from "./Utf8";

/** Text validation shared by settings, network and candidate policies. */
export class TextPolicy {
  /**
   * 按 Unicode 字符（码点）拆开字符串，emoji 这类基本平面以外的字符是一整个，不会拆成两个代理项。
   *
   * 鸿蒙的 ArkTS 运行时上 `Array.from(text)` 和 `[...text]` 都按 UTF-16 码元拆：「a😊b」拆出四段，😊 变成两个落单的代理项，只有 `for...of` 拆出三段（在 HarmonyOS 6.0 模拟器上实测）。Node 上三种写法都是三段，所以单元测试测不出来。拆开后逐个校验的代码因此会把每个 emoji 当成非法字符：AI 对话的回复只要带 emoji 就整条被拒、显示「聊天服务暂不可用」；按字符计数时一个 emoji 算两个。鸿蒙原生代码里按字符处理字符串一律用这个函数，`scripts/test-harmony-string-code-points.py` 拦着 `Array.from` 和整串展开。
   */
  static characters(value: string): string[] {
    const characters: string[] = [];
    for (const character of value) characters.push(character);
    return characters;
  }

  /** Accepts one URL scheme with a non-empty authority before callers apply their own bounds. */
  static hasAuthority(value: string, scheme: string): boolean {
    if (!value.startsWith(scheme)) return false;
    const authority: string = value.substring(scheme.length).split("/")[0].split("?")[0];
    return authority.length > 0;
  }

  /** Validates a bounded endpoint with one of the supplied schemes and no user-info or fragment. */
  static validAuthority(value: string, schemes: string[], maxBytes: number = 2048): boolean {
    return (
      value.length > 0 &&
      utf8Length(value) <= maxBytes &&
      !TextPolicy.hasControl(value) &&
      schemes.some((scheme: string): boolean => TextPolicy.hasAuthority(value, scheme)) &&
      !value.includes("@") &&
      !value.includes("#")
    );
  }

  /** A web URL the settings page may hand to the system browser. */
  static validExternalWebUrl(value: string): boolean {
    return TextPolicy.validAuthority(value, ["https://", "http://"]);
  }

  /** Allows plaintext only for a loopback authority; credentials remain HTTPS-only. */
  static validSecureAuthority(value: string, allowHttp: boolean, maxBytes: number = 2048): boolean {
    if (
      !TextPolicy.validAuthority(
        value,
        allowHttp ? ["https://", "http://"] : ["https://"],
        maxBytes,
      )
    ) {
      return false;
    }
    if (value.startsWith("https://")) return true;
    if (!allowHttp || !value.startsWith("http://")) return false;
    const prefixLength: number = "http://".length;
    const rest: string = value.substring(prefixLength);
    const relativeEnd: number = rest.search(/[\/?#]/);
    const authority: string = rest.substring(0, relativeEnd < 0 ? rest.length : relativeEnd);
    if (authority === "localhost" || authority.startsWith("localhost:")) return true;
    if (authority === "127.0.0.1" || authority.startsWith("127.0.0.1:")) return true;
    return authority === "[::1]" || authority.startsWith("[::1]:");
  }

  /** Rejects malformed UTF-16 and the C0/C1 control ranges. */
  static hasControl(value: string): boolean {
    if (!TextPolicy.validUnicode(value)) return true;
    return TextPolicy.characters(value).some((character: string): boolean => {
      const code: number = character.codePointAt(0) ?? 0;
      return code <= 0x1f || (code >= 0x7f && code <= 0x9f);
    });
  }

  /** 拒绝未配对的 UTF-16 代理项，避免原生或网络文本带着非法 Unicode 进入编辑器。 */
  static validUnicode(value: string): boolean {
    for (let index: number = 0; index < value.length; index++) {
      const unit: number = value.charCodeAt(index);
      if (unit >= 0xd800 && unit <= 0xdbff) {
        if (index + 1 >= value.length) return false;
        const next: number = value.charCodeAt(++index);
        if (next < 0xdc00 || next > 0xdfff) return false;
      } else if (unit >= 0xdc00 && unit <= 0xdfff) {
        return false;
      }
    }
    return true;
  }

  /** Bounds text while allowing the line breaks and tabs used in prompts and transcripts. */
  static validMultiline(value: string, maxBytes: number, requireNonEmpty: boolean): boolean {
    return (
      TextPolicy.validUnicode(value) &&
      (!requireNonEmpty || value.trim().length > 0) &&
      utf8Length(value) <= maxBytes &&
      !TextPolicy.characters(value).some((character: string): boolean => {
        if (character === "\t" || character === "\n" || character === "\r") return false;
        return TextPolicy.hasControl(character);
      })
    );
  }
}
