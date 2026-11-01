/**
 * Display-only double-pinyin key hints derived from the Engine profile tables, ported from
 * platforms/android/java/app/msime/android/ShuangpinKeyHintPolicy.java.
 *
 * These are labels, not behaviour: which unit a key actually produces is the Engine's business. The
 * tables are transcribed from the Java so both hosts print the same thing.
 */
const LETTER_KEYS: string[] = [
  "Q",
  "W",
  "E",
  "R",
  "T",
  "Y",
  "U",
  "I",
  "O",
  "P",
  "A",
  "S",
  "D",
  "F",
  "G",
  "H",
  "J",
  "K",
  "L",
  "Z",
  "X",
  "C",
  "V",
  "B",
  "N",
  "M",
  ";",
];

/** A unit spelled with a leading v is printed with ü, which is what the user is looking for. */
function displayUnit(unit: string): string {
  return unit.startsWith("v") ? "ü" + unit.substring(1) : unit;
}

function addUnits(byKey: Map<string, string[]>, entries: string[]): void {
  for (const entry of entries) {
    const separator: number = entry.indexOf("=");
    const unit: string = displayUnit(entry.substring(0, separator));
    const key: string = entry.substring(separator + 1).toUpperCase();
    const existing: string[] | undefined = byKey.get(key);
    if (existing === undefined) {
      byKey.set(key, [unit]);
    } else {
      existing.push(unit);
    }
  }
}

function join(units: string[] | undefined): string {
  if (units === undefined || units.length === 0) {
    return "";
  }
  return units.slice().sort().join(" ");
}

function profile(initials: string[], finals: string[]): Map<string, string> {
  const initialsByKey: Map<string, string[]> = new Map<string, string[]>();
  const finalsByKey: Map<string, string[]> = new Map<string, string[]>();
  addUnits(initialsByKey, initials);
  addUnits(finalsByKey, finals);
  const hints: Map<string, string> = new Map<string, string>();
  for (const key of LETTER_KEYS) {
    const initial: string = join(initialsByKey.get(key));
    const ending: string = join(finalsByKey.get(key));
    if (initial.length === 0 && ending.length === 0) {
      continue;
    }
    hints.set(
      key,
      initial.length === 0 ? ending : ending.length === 0 ? initial : initial + " / " + ending,
    );
  }
  return hints;
}

/** 粤语双拼键面的名字，与引擎的保留名相同；它不是普通话双拼方案名，会话的 `shuangpinProfile` 里不会出现。 */
export const CANTONESE_PROFILE: string = "cantonese";
/** 粤语双拼的触屏方案 id（`KeyboardScheme.CANTONESE_SHUANGPIN.preferenceId`）。 */
const CANTONESE_SHUANGPIN_SCHEME: string = "cantonese_shuangpin";
/** 会话 `scheme` 里粤拼的编号，普通粤拼和粤语双拼都跑在它上面。 */
const CANTONESE_ENGINE_SCHEME: number = 5;

const PROFILES: Map<string, Map<string, string>> = new Map<string, Map<string, string>>([
  [
    "xiaohe",
    profile(
      ["sh=u", "ch=i", "zh=v"],
      [
        "iu=q",
        "ei=w",
        "e=e",
        "uan=r",
        "ue=t",
        "ve=t",
        "un=y",
        "u=u",
        "i=i",
        "uo=o",
        "o=o",
        "ie=p",
        "a=a",
        "ong=s",
        "iong=s",
        "ai=d",
        "en=f",
        "eng=g",
        "ang=h",
        "an=j",
        "uai=k",
        "ing=k",
        "uang=l",
        "iang=l",
        "ou=z",
        "ua=x",
        "ia=x",
        "ao=c",
        "ui=v",
        "v=v",
        "in=b",
        "iao=n",
        "ian=m",
      ],
    ),
  ],
  [
    "ziranma",
    profile(
      ["sh=u", "ch=i", "zh=v"],
      [
        "iu=q",
        "ia=w",
        "ua=w",
        "e=e",
        "uan=r",
        "ue=t",
        "ve=t",
        "ing=y",
        "uai=y",
        "u=u",
        "i=i",
        "o=o",
        "uo=o",
        "un=p",
        "a=a",
        "iong=s",
        "ong=s",
        "iang=d",
        "uang=d",
        "en=f",
        "eng=g",
        "ang=h",
        "an=j",
        "ao=k",
        "ai=l",
        "ei=z",
        "ie=x",
        "iao=c",
        "ui=v",
        "v=v",
        "ou=b",
        "in=n",
        "ian=m",
      ],
    ),
  ],
  [
    "shoudao",
    profile(
      ["sh=e", "ch=i", "zh=v"],
      [
        "iu=q",
        "ua=w",
        "e=e",
        "ie=r",
        "uan=t",
        "ang=y",
        "u=u",
        "i=i",
        "o=o",
        "uo=o",
        "iao=p",
        "a=a",
        "ou=s",
        "ao=d",
        "eng=f",
        "uai=g",
        "ing=g",
        "ong=h",
        "iong=h",
        "an=j",
        "en=k",
        "ia=k",
        "ai=l",
        "ue=l",
        "un=z",
        "iang=x",
        "uang=x",
        "in=c",
        "v=v",
        "ui=v",
        "ve=b",
        "ian=n",
        "ei=m",
      ],
    ),
  ],
  [
    "microsoft",
    profile(
      ["sh=u", "ch=i", "zh=v"],
      [
        "iu=q",
        "ia=w",
        "ua=w",
        "e=e",
        "uan=r",
        "ue=t",
        "ve=v",
        "uai=y",
        "v=y",
        "u=u",
        "i=i",
        "o=o",
        "uo=o",
        "un=p",
        "a=a",
        "iong=s",
        "ong=s",
        "iang=d",
        "uang=d",
        "en=f",
        "eng=g",
        "ang=h",
        "an=j",
        "ao=k",
        "ai=l",
        "ing=;",
        "ei=z",
        "ie=x",
        "iao=c",
        "ui=v",
        "ou=b",
        "in=n",
        "ian=m",
      ],
    ),
  ],
  // 粤语双拼（#10438）：引擎把它的键面放在保留名 `cantonese` 下（`host::CANTONESE_SHUANGPIN_HINTS`），表抄自 `cantonese::shuangpin` 的 `MOVED_INITIALS` 和 `FINALS`。声母只列不在自己字母键上的 gw、kw、ng、j；零声母键 o 和鼻音音节 m、ng 的码不写。
  [
    CANTONESE_PROFILE,
    profile(
      ["gw=i", "kw=u", "ng=v", "j=y"],
      [
        "a=a",
        "aa=a",
        "ou=b",
        "aam=c",
        "aap=c",
        "am=d",
        "ap=d",
        "e=e",
        "an=f",
        "at=f",
        "ang=g",
        "ak=g",
        "eng=g",
        "aang=h",
        "aak=h",
        "i=i",
        "aan=j",
        "aat=j",
        "aau=k",
        "au=k",
        "aai=l",
        "ai=l",
        "im=m",
        "ip=m",
        "in=n",
        "o=o",
        "oi=o",
        "un=p",
        "ut=p",
        "ung=p",
        "eon=p",
        "iu=q",
        "yun=r",
        "yut=r",
        "uk=r",
        "on=s",
        "ong=s",
        "ok=s",
        "ot=s",
        "it=t",
        "ek=t",
        "u=u",
        "yu=v",
        "ui=v",
        "oe=w",
        "oeng=x",
        "oek=x",
        "eoi=y",
        "eot=y",
        "ei=z",
        "ing=;",
        "ik=;",
      ],
    ),
  ],
]);

export class ShuangpinKeyHintPolicy {
  /**
   * 是否画粤语双拼的键面：选中的触屏方案是粤语双拼、会话也确实跑着粤拼。普通粤拼与粤语双拼是同一个引擎方案，只有键盘知道选的是哪一个，普通粤拼完整拼写，不画；会话跑的不是粤拼时（缺了 `msime-cantonese.db`，运行时留在上一个中文方案）不画，免得把键标成没在跑的方案。与 Android 的 `ShuangpinKeyHintPolicy.rememberedProfile`、iOS 的 `cantoneseShuangpinKeyHints` 一致。
   */
  static cantoneseFace(touchScheme: string, engineScheme: number): boolean {
    return touchScheme === CANTONESE_SHUANGPIN_SCHEME && engineScheme === CANTONESE_ENGINE_SCHEME;
  }

  /** `enabled` 是共享偏好 `touch_shuangpin_key_hints`：用户关掉后双拼和粤语双拼也不画提示，其余条件与它无关。 */
  static visible(
    dedicatedEnglish: boolean,
    scheme: number,
    localMode: string,
    enabled: boolean,
  ): boolean {
    return enabled && !dedicatedEnglish && scheme === 1 && localMode === "none";
  }

  static hint(
    profileName: string,
    key: string | null,
    dedicatedEnglish: boolean,
    scheme: number,
    localMode: string,
    enabled: boolean,
  ): string {
    if (
      !ShuangpinKeyHintPolicy.visible(dedicatedEnglish, scheme, localMode, enabled) ||
      key === null
    ) {
      return "";
    }
    const hints: Map<string, string> | undefined = PROFILES.get(profileName);
    if (hints === undefined) {
      return "";
    }
    const hint: string | undefined = hints.get(key.toUpperCase());
    return hint === undefined ? "" : hint;
  }
}
