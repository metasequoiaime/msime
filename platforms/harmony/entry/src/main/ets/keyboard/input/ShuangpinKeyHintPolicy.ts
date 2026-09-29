/**
 * Display-only double-pinyin key hints derived from the Engine profile tables, ported from
 * platforms/android/java/app/msime/client/ShuangpinKeyHintPolicy.java.
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
]);

export class ShuangpinKeyHintPolicy {
  static visible(dedicatedEnglish: boolean, scheme: number, localMode: string): boolean {
    return !dedicatedEnglish && scheme === 1 && localMode === "none";
  }

  static hint(
    profileName: string,
    key: string | null,
    dedicatedEnglish: boolean,
    scheme: number,
    localMode: string,
  ): string {
    if (!ShuangpinKeyHintPolicy.visible(dedicatedEnglish, scheme, localMode) || key === null) {
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
