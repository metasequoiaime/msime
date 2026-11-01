#!/usr/bin/env python3
"""为非英文的候选释义行（法、日、西、俄、德、韩）构建离线读音表。

``en-phonetic.db`` 读英文释义行；这里给其余六种释义语言各建一张同样的表，来源与释义本身相同：英文维基词典，经 Wiktextract 抽取、由 kaikki.org 发布。离线释义（``build_offline_glosses.py``）取的是英语词条的译文表，译文行不带读音；读音取的是同一版维基词典里目标语言词条自己的读音，kaikki.org 按语言各发一个 JSONL dump（``kaikki.org-dictionary-French.jsonl.gz`` 等），格式与离线释义读的英语 dump 相同。

读音是这门语言的学习者被教的那种读法：

- 法、西、德、俄用 ``sounds[].ipa``，按维基词典原样保留，连同斜线或方括号。斜线的音位标音优先于方括号的语音标音（西语两种都有，俄语只有方括号），不带地区标签的和标了标准音地区（西班牙、德国……）的优先于其他地区的。法语例外：dump 把标准的法国读音标成 ``France`` 或 ``Europe``，南部变体反而不带标签，所以法语里标了标准音地区的排在不带标签的前面（细节见 ``TAGGED_STANDARD_FIRST``）。德语反过来让方括号优先：斜线是维基词典的音位层（``/ˈhaːbən/``），方括号才是学习者在词典里看到的实际读音（``[ˈhaː.bən]``），词条没有完整的方括号标音时仍给斜线（细节见 ``BRACKETS_FIRST``）。只在联诵时出现的读法（``/tuz‿/``）和只标了一部分的（``[aːt͡s-]``、``[-bn̩]``）不用。
- 韩语用文化观光部式罗马字（Revised Romanization，``goyang-i``、``hakgyo``），与日语给罗马字一样：韩文释义本身就是表音的谚文，学习者用的是罗马字，dump 的窄式 IPA（``[ˈsʰo̞(ː)d͡ʑiɦa̠da̠]``）对他们不友好。过滤后的输入里没有现成的罗马字，按规范从谚文拼写和维基词典写的发音谚文生成（``revised_romanization``，规则和取舍见那一节的说明）。
- 日语用 ``forms[]`` 里标 ``romanization`` 的赫本式罗马字。dump 的 ``sounds`` 有一个很窄的 IPA（``[ne̞ko̞]``），日语学习者从来不学它，还有假名读法（``sounds[].other``）。macOS 和 Windows 早就用系统分词器在日文释义行下面画罗马字，没有分词器的宿主给同一种读音；而且罗马字也能读片假名和平假名的释义行，日文释义大多就是它们，假名读音对它们什么也没加。

几个词条拼法相同时（法语 ``fils``），取最常见词性的第一个词条（按 dump 顺序）的读音；同形词即便读音不同也照样标，因为释义行绝大多数时候是常见义。日语的同形词读音常常完全不同，维基词典又把罕用读法也列成同一词性的词条，而且排在前面的未必是常用的那个（``日`` 先列 ``nichi`` 后列 ``hi``），所以同一词性里先取带假名读法并标了声调的词条（``hi`` 标了 ``[híꜜì]``，``nichi`` 没有），再取带假名读法的（``猫`` 的 ``neko`` 而不是 ``nekoma``），最后才按 dump 顺序。``今日`` 仍会读成 ``konnichi``：这一版 dump 里 ``今日`` 只有 ``konnichi``、``konchi``、``konjitsu`` 三个词条，没有 ``kyō``，macOS 和 Windows 的系统分词器则给 ``kyō``。

输出每种语言一个 SQLite 文件 ``<lang>-phonetic.db``：表 ``<lang>_phonetics(word, phonetic)``，键是维基词典的写法（``Katze``、``кот``、``猫``）；``meta`` 表写明种类（``<lang>_phonetic``）和输入；``PRAGMA user_version = 1``。布局与 ``en-phonetic.db`` 相同，宿主桥接层据此拒绝版本或种类不对的文件。行按键序写入固定页大小的文件并做 VACUUM，同一份输入在同一个 SQLite 版本下得到相同的字节。

表里只收离线释义用到的词（``--glosses``，即 ``resources/offline-glosses.lock.json`` 锁住的 ``zh-<lang>.db``）：每个释义的每个词条，连同它的小写。宿主读的是释义行的第一个词条，绝大多数画出来的行出自这些文件，收全部原形词（六种语言约 13 MB）多半是没有哪一行会用到的词；只收用到的，六张表合计不到 4 MB。在线翻译出来的行，第一个词条是这些词之一时照样有读音。离线释义从更新的 dump 重建时，这些表也从同一周的 dump 重建。

这里不下载任何东西。``filter`` 把六个 dump（约 420 MB）缩成构建要读的字段，这是值得保存的那份文件，因为 kaikki.org 每周覆盖同一个地址；``build`` 读这份缩减后的输入。

维基词典文本是 CC BY-SA 4.0；``build`` 在数据库旁边写 ``target-pronunciations-NOTICE.txt``。

用法：
  build_target_pronunciations.py filter --input <kaikki.org-dictionary-<Language>.jsonl.gz> [--input ...] --out <filtered.jsonl.gz>
  build_target_pronunciations.py build --input <filtered.jsonl[.gz]> --out <directory> [--glosses <offline-glosses directory>] [--lang fr,ja,es,ru,de,ko]
      [--dump-date YYYY-MM-DD] [--source-revision <wiktextract commit>] [--max-bytes N] [--min-words N]
"""
import argparse
import gzip
import json
import re
import sqlite3
import sys
import unicodedata
from pathlib import Path

LANGUAGES = ("fr", "ja", "es", "ru", "de", "ko")
SCHEMA_VERSION = 1
PAGE_SIZE = 4096
MAX_WORD_CHARS = 40
MAX_PHONETIC_CHARS = 60
DEFAULT_MAX_BYTES = 4 << 20
NOTICE_NAME = "target-pronunciations-NOTICE.txt"

# 释义词最可能是的词性，按先后排：释义是名词、动词、形容词的时候远多于其他。没列出的排在这些后面，按 dump 顺序。
POS_ORDER = ("noun", "verb", "adj", "adv", "phrase", "intj", "pron", "num", "prep", "conj", "det", "particle", "name")
# 不是释义会教的词的词条：罗马字页、字母、符号、单独的词缀。
SKIPPED_POS = {"romanization", "character", "symbol", "suffix", "prefix", "infix", "interfix", "circumfix", "affix", "soft-redirect", "punct", "combining_form"}
# 带这些标签的标音不是日常读音。
DROPPED_TAGS = {
    "archaic", "obsolete", "dated", "rare", "dialectal", "nonstandard", "proscribed", "poetic", "literary",
    "historical", "Middle", "Old", "Early",
}
# 各语言学习者被教的标准音所在的地区标签；标了其他地区的标音排在它们后面。法语不收 `Paris`：dump 用它标巴黎口音的特点（`ce` 的 `/sø/`、`tarin` 的 `/täʁæ̃/`），标准读法是不带标签的 `/sə/`、`/ta.ʁɛ̃/`。
STANDARD_REGIONS = {
    "fr": {"France", "Europe", "Standard"},
    "es": {"Spain", "Castilian", "Standard", "Latin-America"},
    "de": {"Germany", "Standard", "Standard-German"},
    "ru": {"Standard"},
    "ko": {"Seoul", "Standard", "SK-Standard"},
    "ja": {"Tokyo", "Standard"},
}
# 地区标签里同时带这些方位词的是某个国家内部的地方变体（法语 `jaune` 标 `France`、`Southern` 的 `/ʒɔn/`），不算标准音。
LOCAL_QUALIFIERS = {"Northern", "Southern", "Eastern", "Western"}
# 这些语言里标了标准音地区的斜线标音排在不带地区标签的前面：法语 dump 把标准的法国读音标成 `France` 或 `Europe`，南部和其他变体反而不带标签（`sept` 是 `/sɛt/` 标 `Europe`、`/sɛtə/` 不带标签，`France` 是 `/fʁɑ̃s/` 标 `France`、`/fʁãⁿsə/` 不带标签）；而它标 `France` 的方括号标音多是窄式记音（`bonjour` 的 `[bõ.ʒuə̯ʁ]`），所以法语先比斜线还是方括号、再比地区。其余语言不带标签的就是标准音（德语 `Chile` 不带标签的 `/ˈt͡ʃiːlə/` 才是通行读法，标 `Germany` 的 `/ˈçiːlə/` 是变体），先比地区：不带标签的、标准音地区的、其他地区的，同一档里斜线优先。
TAGGED_STANDARD_FIRST = {"fr"}
# 这些语言在同一档地区里方括号的语音标音排在斜线的音位标音前面，值是在这些语言里另外丢掉的标签。德语的斜线是维基词典的音位层（`/ˈhaːbən/`），方括号才是学习者在词典里看到的实际读音（`[ˈhaː.bən]`、`er` 的 `[ʔeːɐ̯]`）；没有完整方括号标音的词（`Katze`、`Hund`）仍给斜线。方括号优先以后，标 `colloquial` 的口语缩读（`er` 的 `/ɐ/`、`mal` 的 `/ma/`）不能再排在标准读音前面，所以丢掉。只写在 `sounds[].other` 里的方括号（`Wasser` 的 `[ˈva.sɐ]`）不取：那个字段在德语里多半是用法说明里的片段（`der` 的 `[ˈɪnɐ]` 是 `in der` 的读法），和词条本身的读音分不开。
BRACKETS_FIRST = {"de": {"colloquial"}}
REGIONAL_TAG = re.compile(r"^[A-Z]")
# 读音里除字母外还能出现的字符（任何文字的字母都行；IPA 用拉丁、希腊、IPA 扩展字母和修饰字母）。其他字符（数字、汉字、注释、残留的模板）一律拒掉这条读音，而不是把它显示出来。
READING_MARKS = set(" .ˈˌːˑ‿-‐()[]/|‖ʼ'ʔ̃͜͡")


def open_text(path: Path):
    return gzip.open(path, "rt", encoding="utf-8") if path.suffix == ".gz" else path.open(encoding="utf-8")


def entries(path: Path):
    with open_text(path) as handle:
        for number, line in enumerate(handle, 1):
            line = line.strip()
            if not line:
                continue
            try:
                yield json.loads(line)
            except json.JSONDecodeError as error:
                raise SystemExit(f"{path}:{number}: not JSON ({error})")


def reduce(entry: dict) -> dict | None:
    """构建要读的字段；构建用不到的词条返回 None。"""
    language = entry.get("lang_code")
    word = entry.get("word")
    if language not in LANGUAGES or not isinstance(word, str) or not word.strip():
        return None
    sounds = []
    for sound in entry.get("sounds") or []:
        if not isinstance(sound, dict):
            continue
        kept = {key: sound[key] for key in ("ipa", "other", "tags") if key in sound}
        if "ipa" in kept or "other" in kept:
            sounds.append(kept)
    romanizations = [
        form["form"]
        for form in entry.get("forms") or []
        if language == "ja" and isinstance(form, dict) and isinstance(form.get("form"), str) and form.get("tags") == ["romanization"]
    ]
    if not sounds and not romanizations:
        return None
    reduced = {"word": word, "lang_code": language, "pos": entry.get("pos", ""), "sounds": sounds}
    senses = [sense for sense in entry.get("senses") or [] if isinstance(sense, dict)]
    # 屈折形式（俄语 кошки、法语 chattes）是单独的词条，有自己的 IPA；释义写的是原形，所以这里打上标记，构建时丢掉。
    if senses and all("form_of" in sense or "form-of" in (sense.get("tags") or []) for sense in senses):
        reduced["form_of"] = True
    if romanizations:
        reduced["romanizations"] = romanizations
    return reduced


def filter_command(arguments) -> int:
    kept = {language: 0 for language in LANGUAGES}
    with arguments.out.open("wb") as raw:
        # mtime=0 且不写文件名，gzip 输出每次运行逐字节相同。
        output = gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) if arguments.out.suffix == ".gz" else raw
        with output:
            for path in arguments.input:
                for entry in entries(path):
                    reduced = reduce(entry)
                    if reduced is None:
                        continue
                    output.write((json.dumps(reduced, ensure_ascii=False, separators=(",", ":"), sort_keys=True) + "\n").encode("utf-8"))
                    kept[reduced["lang_code"]] += 1
    for language, count in kept.items():
        print(f"{language} {count:8} entries")
    if not any(kept.values()):
        raise SystemExit("no entry of fr/ja/es/ru/de/ko carries a pronunciation; the source layout changed")
    print(f"-> {arguments.out}")
    return 0


def usable_word(word: str) -> bool:
    return (
        0 < len(word) <= MAX_WORD_CHARS
        and word == word.strip()
        and not any(unicodedata.category(character)[0] in "CZ" and character != " " for character in word)
        and ";" not in word
    )


def clean_ipa(raw: str) -> str | None:
    """按维基词典原样返回标音；不是单独一条纯 IPA 标音时返回 None。"""
    text = unicodedata.normalize("NFC", " ".join(raw.split()))
    if not text or len(text) > MAX_PHONETIC_CHARS:
        return None
    # 一格里两条标音（"/ʃa/, /ʃat/"）或带注释（"/ʃa/ (Quebec)"）都不是一个读音。
    if "," in text or ";" in text or "~" in text:
        return None
    opening, closing = text[0], text[-1]
    if (opening, closing) not in (("/", "/"), ("[", "]")) or len(text) < 3:
        return None
    inner = text[1:-1]
    if "/" in inner or "[" in inner or "]" in inner or inner.count("(") != inner.count(")"):
        return None
    # 以联诵符开头或结尾的是只在联诵时出现的读法（`tous` 的 `/tuz‿/`、`cet` 的 `/sɛ.t‿/`），单独念这个词时不这么读；以连字符开头或结尾的只标了词的一部分（德语 `Arzneimittel` 的 `[aːt͡s-]`）。
    if inner[:1] in ("‿", "-", "‐") or inner[-1:] in ("‿", "-", "‐"):
        return None
    for character in inner:
        category = unicodedata.category(character)
        if category[0] == "L" and not _han_or_kana(character):
            continue
        if category in ("Mn", "Lm", "Sk") or character in READING_MARKS:
            continue
        return None
    return text


def _han_or_kana(character: str) -> bool:
    name = unicodedata.name(character, "")
    return name.startswith(("CJK", "HIRAGANA", "KATAKANA", "HANGUL"))


def clean_romaji(raw: str) -> str | None:
    """赫本式罗马字（``neko``、``ōkii``、``konnichi wa``）；其他内容返回 None。"""
    text = unicodedata.normalize("NFC", " ".join(raw.split()))
    if not text or len(text) > MAX_PHONETIC_CHARS:
        return None
    for character in text:
        if character in " -'’.":
            continue
        base = unicodedata.normalize("NFD", character)[0]
        if not ("a" <= base.lower() <= "z"):
            return None
    return text if any(character.isalpha() for character in text) else None


def ipa_rank(sound: dict, language: str) -> int | None:
    """一个词条里各条标音的先后，小的优先；不能用的返回 None。比较顺序见 `TAGGED_STANDARD_FIRST` 的说明。"""
    tags = sound.get("tags") or []
    if not isinstance(tags, list) or any(tag in DROPPED_TAGS for tag in tags):
        return None
    regional = [tag for tag in tags if isinstance(tag, str) and REGIONAL_TAG.match(tag)]
    standard = any(tag in STANDARD_REGIONS.get(language, set()) for tag in regional) and not any(tag in LOCAL_QUALIFIERS for tag in regional)
    if language in BRACKETS_FIRST and any(tag in BRACKETS_FIRST[language] for tag in tags):
        return None
    phonetic = 1 if str(sound.get("ipa", "")).startswith("[") else 0
    if language in BRACKETS_FIRST:
        phonetic = 1 - phonetic
    if language in TAGGED_STANDARD_FIRST:
        region = 0 if standard else 1 if not regional else 2
        return phonetic * 3 + region
    region = 0 if not regional else 1 if standard else 2
    return region * 2 + phonetic


# ---- 韩语的文化观光部式罗马字（Revised Romanization of Korean，2000 年文化观光部告示第 2000-8 号） ----
#
# 过滤后的输入里韩语词条没有现成的罗马字：kaikki 的韩语 dump 把维基词典的罗马字写在 `forms[]` 里，`filter` 只给日语留了 `forms`，锁住的那一周的原始 dump 已被 kaikki.org 覆盖。输入里有的是维基词典按标准发音写的谚文（`sounds[].other`，`읽다` 的 `[익따]`、`국물` 的 `[궁물]`、`한국어` 的 `[한(ː)구거]`），所以罗马字在这里按规范的规则从谚文生成：
#
# - 罗马字按发音写（规范第 3 章第 1 项），发音用维基词典写的那一条：连音、鼻音化、流音化、送气、腭化和 ㄴ 添加都已经在里面（`환율` 的 `[화뉼]`、`솜이불` 的 `[솜니불]`），这些靠拼写推不出来。
# - 紧音化不写（同项附注）：发音里的紧音声母在拼写里是对应的松音时按松音写，`학교` 的 `[학꾜]` 是 `hakgyo`。拼写里本来就是紧音的（`깎아` 的 `[까까]`、`있어` 的 `[이써]`）照写。
# - 元音按拼写写，不按发音：`희망` 读 `[히망]` 仍写 `huimang`（规范第 2 章附注 1），`차례` 有 `[차레]` 的读法仍写 `charye`。
# - 体词里 ㄱ、ㄷ、ㅂ 后面跟 ㅎ 时 ㅎ 照写（同项附注），`묵호` 是 `Mukho` 不是 `Muko`；谓词照常送气，`좋다` 是 `jota`。
# - 音节界线可能读错的地方加连字符（第 3 章第 2 项）：`중앙` 是 `jung-ang`，`반구대` 是 `ban-gudae`，`해운대` 是 `hae-undae`。维基词典在同样的地方用撇号（`goyang'i`），这里按规范用连字符。
# - 专名（词性 `name`）首字母大写（第 3 章第 3 项），`신라` 是 `Silla`。
#
# 维基词典有一部分词条不写发音谚文（`개`、`책`、`세계` 这类元音有 ㅐ/ㅔ 两读、维基词典只给 IPA 的），这时按标准发音法从拼写推出发音（`spelled_pronunciation`），规则只到不需要知道词的构成为止：ㅎ 收音、送气、连音、腭化、收音代表音、鼻音化、ㄹ 的鼻音化和流音化。ㄴ 添加、合成词里的连音（`맛없다`）推不出来，这类词出错的余地在这里。词头不是谚文（汉字、拉丁字母）的词条不标读音：发音谚文和拼写对不上，紧音化无从判断。

HANGUL_INITIALS = "ㄱㄲㄴㄷㄸㄹㅁㅂㅃㅅㅆㅇㅈㅉㅊㅋㅌㅍㅎ"
HANGUL_VOWELS = ("ㅏ", "ㅐ", "ㅑ", "ㅒ", "ㅓ", "ㅔ", "ㅕ", "ㅖ", "ㅗ", "ㅘ", "ㅙ", "ㅚ", "ㅛ", "ㅜ", "ㅝ", "ㅞ", "ㅟ", "ㅠ", "ㅡ", "ㅢ", "ㅣ")
HANGUL_FINALS = ("", "ㄱ", "ㄲ", "ㄳ", "ㄴ", "ㄵ", "ㄶ", "ㄷ", "ㄹ", "ㄺ", "ㄻ", "ㄼ", "ㄽ", "ㄾ", "ㄿ", "ㅀ", "ㅁ", "ㅂ", "ㅄ", "ㅅ", "ㅆ", "ㅇ", "ㅈ", "ㅊ", "ㅋ", "ㅌ", "ㅍ", "ㅎ")
RR_INITIALS = dict(zip(HANGUL_INITIALS, ("g", "kk", "n", "d", "tt", "r", "m", "b", "pp", "s", "ss", "", "j", "jj", "ch", "k", "t", "p", "h")))
RR_VOWELS = dict(zip(HANGUL_VOWELS, ("a", "ae", "ya", "yae", "eo", "e", "yeo", "ye", "o", "wa", "wae", "oe", "yo", "u", "wo", "we", "wi", "yu", "eu", "ui", "i")))
RR_FINALS = {"": "", "ㄱ": "k", "ㄴ": "n", "ㄷ": "t", "ㄹ": "l", "ㅁ": "m", "ㅂ": "p", "ㅇ": "ng"}
# 收音的代表音（标准发音法第 9 至 11 条）；ㄺ 和 ㄼ 取多数词的读法（`닭` 是 `[닥]`，`여덟` 是 `[여덜]`）。
NEUTRAL_FINALS = {
    "": "", "ㄱ": "ㄱ", "ㄲ": "ㄱ", "ㅋ": "ㄱ", "ㄳ": "ㄱ", "ㄺ": "ㄱ", "ㄴ": "ㄴ", "ㄵ": "ㄴ", "ㄶ": "ㄴ",
    "ㄷ": "ㄷ", "ㅅ": "ㄷ", "ㅆ": "ㄷ", "ㅈ": "ㄷ", "ㅊ": "ㄷ", "ㅌ": "ㄷ", "ㅎ": "ㄷ",
    "ㄹ": "ㄹ", "ㄼ": "ㄹ", "ㄽ": "ㄹ", "ㄾ": "ㄹ", "ㅀ": "ㄹ", "ㅁ": "ㅁ", "ㄻ": "ㅁ", "ㅂ": "ㅂ", "ㅍ": "ㅂ", "ㅄ": "ㅂ", "ㄿ": "ㅂ", "ㅇ": "ㅇ",
}
# 双收音在连音时留下的前一半和移到下一音节的后一半。
SPLIT_FINALS = {"ㄳ": ("ㄱ", "ㅅ"), "ㄵ": ("ㄴ", "ㅈ"), "ㄺ": ("ㄹ", "ㄱ"), "ㄻ": ("ㄹ", "ㅁ"), "ㄼ": ("ㄹ", "ㅂ"), "ㄽ": ("ㄹ", "ㅅ"), "ㄾ": ("ㄹ", "ㅌ"), "ㄿ": ("ㄹ", "ㅍ"), "ㅄ": ("ㅂ", "ㅅ")}
ASPIRATED = {"ㄱ": "ㅋ", "ㄷ": "ㅌ", "ㅂ": "ㅍ", "ㅈ": "ㅊ"}
TENSE_TO_LAX = {"ㄲ": "ㄱ", "ㄸ": "ㄷ", "ㅃ": "ㅂ", "ㅆ": "ㅅ", "ㅉ": "ㅈ"}
NASALIZED = {"ㄱ": "ㅇ", "ㄷ": "ㄴ", "ㅂ": "ㅁ"}
# 拼起来以后会被读成另一个元音的音节交界：`세운` 的 `e` + `u` 会读成 `eu`。
AMBIGUOUS_VOWEL_JOINS = {"ae", "eo", "eu", "oe", "ui"}


def hangul_syllable(character: str) -> list[str] | None:
    """一个谚文音节的 [声母, 元音, 收音]（收音可以是空串）；不是谚文音节时返回 None。"""
    index = ord(character) - 0xAC00
    if not 0 <= index < 11172:
        return None
    return [HANGUL_INITIALS[index // 588], HANGUL_VOWELS[index % 588 // 28], HANGUL_FINALS[index % 28]]


def spelled_pronunciation(word: list[list[str]]) -> list[list[str]]:
    """按标准发音法从拼写推出的发音，一个空格分开的词为单位；规则见上面的说明。"""
    syllables = [list(syllable) for syllable in word]
    for index in range(len(syllables) - 1):
        current, following = syllables[index], syllables[index + 1]
        final, initial = current[2], following[0]
        # ㅎ 收音（第 12 条）：与后面的 ㄱ、ㄷ、ㅈ 合成送气音，在 ㅅ 前和元音前不发音，在 ㄴ 前读 ㄴ。
        if final in ("ㅎ", "ㄶ", "ㅀ"):
            rest = {"ㅎ": "", "ㄶ": "ㄴ", "ㅀ": "ㄹ"}[final]
            if initial in ("ㄱ", "ㄷ", "ㅈ"):
                following[0] = ASPIRATED[initial]
                current[2] = rest
            elif initial in ("ㅅ", "ㅇ"):
                current[2] = rest
            elif initial == "ㄴ":
                current[2] = rest or "ㄴ"
            final, initial = current[2], following[0]
        # ㄱ、ㄷ、ㅂ、ㅈ 与后面的 ㅎ 合成送气音（第 12 条附注 1）。
        if initial == "ㅎ" and final:
            kept, moved = SPLIT_FINALS.get(final, ("", final))
            base = "ㅈ" if moved in ("ㅈ", "ㅊ") else NEUTRAL_FINALS[moved]
            if base in ASPIRATED:
                current[2] = kept
                following[0] = ASPIRATED[base]
                if following[0] == "ㅌ" and following[1] == "ㅣ":
                    following[0] = "ㅊ"
                continue
        # 连音（第 13、14 条）和腭化（第 17 条）。
        if initial == "ㅇ" and final and final != "ㅇ":
            kept, moved = SPLIT_FINALS.get(final, ("", final))
            if moved in ("ㄷ", "ㅌ") and following[1] == "ㅣ":
                moved = "ㅈ" if moved == "ㄷ" else "ㅊ"
            current[2], following[0] = kept, moved
            continue
        final = current[2] = NEUTRAL_FINALS[final]
        # ㄹ 在 ㅁ、ㅇ 后读 ㄴ，在 ㄱ、ㄷ、ㅂ 后也读 ㄴ（第 19 条）；ㄴ 与 ㄹ 相邻读 ㄹㄹ（第 20 条）。
        if initial == "ㄹ":
            if final in ("ㅁ", "ㅇ") or final in NASALIZED:
                following[0] = initial = "ㄴ"
            elif final == "ㄴ":
                current[2] = final = "ㄹ"
        elif initial == "ㄴ" and final == "ㄹ":
            following[0] = initial = "ㄹ"
        # 鼻音化（第 18 条）。
        if initial in ("ㄴ", "ㅁ") and final in NASALIZED:
            current[2] = NASALIZED[final]
    if syllables:
        syllables[-1][2] = NEUTRAL_FINALS[syllables[-1][2]]
    return syllables


def pronounced_hangul(entry: dict, length: int) -> list[list[str]] | None:
    """维基词典写的发音谚文（`[익따]`），去掉可选长音记号，按音节拆开；第一条音节数与拼写相同的才用。"""
    for sound in entry.get("sounds") or []:
        other = sound.get("other") if isinstance(sound, dict) else None
        if not isinstance(other, str) or sound.get("tags") or not (other.startswith("[") and other.endswith("]")):
            continue
        text = other[1:-1].replace("(ː)", "").replace("ː", "").replace(" ", "")
        syllables = [hangul_syllable(character) for character in text]
        if syllables and all(syllables) and len(syllables) == length:
            return syllables
    return None


def revised_romanization(word: str, entry: dict) -> str | None:
    """谚文词头 ``word`` 的罗马字；词头不全是谚文和空格时返回 None。"""
    parts = word.split(" ")
    spelled = [[hangul_syllable(character) for character in part] for part in parts]
    if not all(spelled) or not all(all(part) for part in spelled):
        return None
    flat = [syllable for part in spelled for syllable in part]
    rules = [syllable for part in spelled for syllable in spelled_pronunciation(part)]
    pronounced = pronounced_hangul(entry, len(flat))
    if pronounced is None:
        pronounced = rules
    else:
        # 罗马字按词分写，词与词之间的连音和同化（`누워서 떡 먹기` 的 `[떵 먹끼]`）不写：每个词的末音节收音和下一个词的首音节声母按单独读这个词时的发音。
        start = 0
        for part in spelled[:-1]:
            start += len(part)
            pronounced[start - 1][2] = rules[start - 1][2]
            pronounced[start][0] = rules[start][0]
    noun = entry.get("pos") in ("noun", "name")
    words, position = [], 0
    for part in spelled:
        letters = []
        for offset in range(len(part)):
            index = position + offset
            initial, _, final = pronounced[index]
            vowel = flat[index][1]
            spelled_initial = flat[index][0]
            spelled_before = flat[index - 1][2] if offset else ""
            # 紧音化不写：发音是紧音、拼写不是它的声母按拼写写（`학꾜` 的 `꾜` 写 `gyo`，外来词的口语紧音 `카페` 的 `[까페]` 也写 `kape`）；拼写是 ㅇ 时紧音是前一个收音移过来的，前一个收音本来就是紧音才照写（`있어` 是 `isseo`，`없이` 的 `[업씨]` 是 `eopsi`）。
            if initial in TENSE_TO_LAX and spelled_initial != initial:
                if spelled_initial != "ㅇ":
                    initial = spelled_initial
                elif SPLIT_FINALS.get(spelled_before, ("", spelled_before))[1] != initial:
                    initial = TENSE_TO_LAX[initial]
            # ㅎ 在 ㄴ、ㄹ、ㅁ、ㅇ 和元音后面按标准发音照读（第 12 条只让 ㅎ 收音在元音前不发音），维基词典写出的弱化读法（`전화` 的 `[저놔]`）不算：恢复 ㅎ 和被它带走的收音。
            if spelled_initial == "ㅎ" and initial not in ("ㅎ", "ㅋ", "ㅌ", "ㅍ", "ㅊ") and spelled_before in ("", "ㄴ", "ㄹ", "ㅁ", "ㅇ"):
                initial = "ㅎ"
                if letters:
                    text, before_initial, _ = letters[-1]
                    letters[-1] = (text, before_initial, spelled_before)
            final = NEUTRAL_FINALS.get(final, "")
            if offset + 1 < len(part):
                following = pronounced[index + 1]
                # 体词里 ㄱ、ㄷ、ㅂ 后面的 ㅎ 照写：恢复被送气吞掉的收音和 ㅎ。
                if noun and flat[index][2] in ("ㄱ", "ㄷ", "ㅂ") and flat[index + 1][0] == "ㅎ" and following[0] in ("ㅋ", "ㅌ", "ㅍ", "ㅊ") and not final:
                    final = flat[index][2]
                    pronounced[index + 1] = ["ㅎ", following[1], following[2]]
            previous_final = letters[-1][2] if letters else ""
            text = ("l" if initial == "ㄹ" and previous_final == "ㄹ" else RR_INITIALS[initial]) + RR_VOWELS[vowel]
            letters.append((text, initial, final))
        romanized = ""
        for offset, (text, initial, final) in enumerate(letters):
            if offset:
                before = romanized[-1]
                if (initial == "ㅇ" and (letters[offset - 1][2] == "ㅇ" or (not letters[offset - 1][2] and before + text[0] in AMBIGUOUS_VOWEL_JOINS))) or (letters[offset - 1][2] == "ㄴ" and initial == "ㄱ"):
                    romanized += "-"
            romanized += text + RR_FINALS[final]
        words.append(romanized)
        position += len(part)
    reading = " ".join(words)
    if entry.get("pos") == "name":
        reading = reading[:1].upper() + reading[1:]
    return reading if reading and len(reading) <= MAX_PHONETIC_CHARS else None


def entry_reading(entry: dict, language: str) -> str | None:
    if language == "ko":
        return revised_romanization(unicodedata.normalize("NFC", entry.get("word", "")), entry)
    if language == "ja":
        for form in entry.get("romanizations") or []:
            reading = clean_romaji(form)
            if reading:
                return reading
        return None
    ranked = []
    for position, sound in enumerate(entry.get("sounds") or []):
        if not isinstance(sound, dict) or not isinstance(sound.get("ipa"), str):
            continue
        order = ipa_rank(sound, language)
        reading = clean_ipa(sound["ipa"])
        if order is not None and reading:
            ranked.append((order, position, reading))
    return min(ranked)[2] if ranked else None


def is_kana(text: str) -> bool:
    return bool(text) and all("\u3041" <= character <= "\u309f" or "\u30a1" <= character <= "\u30fa" or character == "ー" for character in text)


def hiragana(text: str) -> str:
    """片假名折成平假名（リンゴ -> りんご）；长音符和平假名不变。"""
    return "".join(chr(ord(character) - 0x60) if "\u30a1" <= character <= "\u30f6" else character for character in text)


def pos_rank(pos: str) -> int:
    return POS_ORDER.index(pos) if pos in POS_ORDER else len(POS_ORDER)


def japanese_strength(entry: dict) -> int:
    """同一词性的几个日语同形词条里，哪一个更像常用读音，大的优先：带假名读法的（`日` 的 `hi` 和 `nichi` 都带，`猫` 的 `nekoma` 不带）胜过不带的，其中又标了声调（`[híꜜì]`）的胜过没标的。维基词典只给常用的和语词标声调，罕用读法多半连假名都没有。"""
    kana = accent = False
    for sound in entry.get("sounds") or []:
        other = sound.get("other") if isinstance(sound, dict) else None
        if not isinstance(other, str) or sound.get("tags"):
            continue
        kana = kana or is_kana(other)
        accent = accent or (other.startswith("[") and other.endswith("]") and any("a" <= character.lower() <= "z" for character in other))
    return kana + accent


def collect(path: Path, languages) -> dict[str, dict[str, str]]:
    """每种语言的「词 -> 读音」。"""
    # 每个键的当前最优：(排序键, 读音)。排序键是 (词头还是假名, 词性, 日语词条的强弱, dump 顺序)。
    best: dict[str, dict[str, tuple]] = {language: {} for language in languages}
    for sequence, entry in enumerate(entries(path)):
        language = entry.get("lang_code")
        word = entry.get("word")
        if language not in best or not isinstance(word, str):
            continue
        pos = entry.get("pos", "")
        if pos in SKIPPED_POS or entry.get("form_of"):
            continue
        word = unicodedata.normalize("NFC", word)
        if not usable_word(word):
            continue
        reading = entry_reading(entry, language)
        if not reading:
            continue
        strength = -japanese_strength(entry) if language == "ja" else 0
        keys = [((0, pos_rank(pos), strength, sequence), word)]
        # 日语词条也给它的假名写法一个读音（林檎是りんご，ringo），排在所有词头后面，这样用假名写的释义（リンゴ、りんご）也有读音：纯假名的词条多半只是指向汉字词条，自己不带读音。
        if language == "ja":
            kana = {sound["other"] for sound in entry.get("sounds") or [] if isinstance(sound.get("other"), str) and is_kana(sound["other"])}
            if len(kana) == 1:
                keys.append(((1, pos_rank(pos), strength, sequence), hiragana(kana.pop())))
        for order, key in keys:
            current = best[language].get(key)
            if current is None or order < current[0]:
                best[language][key] = (order, reading)
    return {language: {word: reading for word, (_, reading) in found.items()} for language, found in best.items()}


GLOSS_SEPARATORS = re.compile(r"[;；,，、/]")


def gloss_vocabulary(directory: Path, language: str) -> set[str]:
    """``zh-<language>.db`` 里每个释义的每个词条，连同它的小写。"""
    path = directory / f"zh-{language}.db"
    if not path.is_file():
        raise SystemExit(f"{path}: missing; --glosses takes the directory scripts/fetch_offline_glosses.py writes")
    terms = set()
    with sqlite3.connect(f"file:{path}?mode=ro", uri=True) as database:
        stored = database.execute("SELECT value FROM meta WHERE key = 'target_language'").fetchone()
        if stored != (language,):
            raise SystemExit(f"{path}: not the {language} glosses")
        for (gloss,) in database.execute("SELECT gloss FROM zh_glosses"):
            for term in GLOSS_SEPARATORS.split(gloss):
                term = unicodedata.normalize("NFC", term.strip())
                if term:
                    terms.add(term)
                    terms.add(term.lower())
    return terms


def write_database(path: Path, language: str, rows: dict[str, str], meta: dict) -> None:
    path.unlink(missing_ok=True)
    database = sqlite3.connect(path)
    try:
        database.execute(f"PRAGMA page_size = {PAGE_SIZE}")
        database.execute("PRAGMA journal_mode = DELETE")
        database.execute(f"CREATE TABLE {language}_phonetics(word TEXT PRIMARY KEY, phonetic TEXT NOT NULL) WITHOUT ROWID")
        database.execute("CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID")
        database.executemany(f"INSERT INTO {language}_phonetics VALUES (?, ?)", sorted(rows.items()))
        values = {**meta, "kind": f"{language}_phonetic", "language": language, "key_count": str(len(rows))}
        database.executemany("INSERT INTO meta VALUES (?, ?)", sorted(values.items()))
        database.execute(f"PRAGMA user_version = {SCHEMA_VERSION}")
        database.commit()
        database.execute("VACUUM")
    finally:
        database.close()


NOTICE = """Candidate gloss readings ({files})

These files contain pronunciations taken from the English Wiktionary (https://en.wiktionary.org/), by Wiktionary contributors, as extracted by Wiktextract (https://github.com/tatuylonen/wiktextract) and published by kaikki.org (https://kaikki.org/dictionary/). Each row comes from the Wiktionary page of its word, https://en.wiktionary.org/wiki/<word>; a Japanese word written in kana takes the reading of the entry whose kana spelling it is (リンゴ from 林檎).

Wiktionary edition: English
Wiktextract dump date: {dump_date}
Wiktextract revision: {source_revision}

Changes: only the IPA transcriptions (French, Spanish, German, Russian; for German the phonetic transcription in square brackets where the entry gives one) and the Hepburn romanization (Japanese) of each word were kept, one per word; the Korean readings are the Revised Romanization of Korean, generated from the Hangul spelling and the Hangul pronunciation the entry gives; archaic, dialectal and similar transcriptions, transcriptions holding more than one variant, liaison-only forms and partial transcriptions were removed; inflected forms were removed and only the words used by the offline candidate glosses were kept; the result was converted to SQLite.

These files are licensed under the Creative Commons Attribution-ShareAlike 4.0 International License (https://creativecommons.org/licenses/by-sa/4.0/), the license of the Wiktionary text they adapt. They are provided as is, without warranties of any kind.
"""


def gloss_dump_date(directory: Path) -> str:
    """``directory`` 里的离线释义是从哪一份 dump 构建的，按它们的 ``meta`` 记录。"""
    dates = set()
    for path in sorted(directory.glob("zh-*.db")):
        with sqlite3.connect(f"file:{path}?mode=ro", uri=True) as database:
            row = database.execute("SELECT value FROM meta WHERE key = 'dump_date'").fetchone()
            dates.add(row[0] if row else "unknown")
    if len(dates) != 1:
        raise SystemExit(f"{directory}: the gloss files come from {len(dates)} dumps")
    return dates.pop()


def build_command(arguments) -> int:
    languages = tuple(dict.fromkeys(language.strip() for language in arguments.lang.split(",") if language.strip()))
    unknown = [language for language in languages if language not in LANGUAGES]
    if not languages or unknown:
        raise SystemExit(f"--lang takes a subset of {','.join(LANGUAGES)}")
    found = collect(arguments.input, languages)
    arguments.out.mkdir(parents=True, exist_ok=True)
    meta = {
        "source": "https://kaikki.org/dictionary/",
        "edition": "en",
        "license": "CC-BY-SA-4.0",
        "dump_date": arguments.dump_date,
        "source_revision": arguments.source_revision,
        "sqlite_version": sqlite3.sqlite_version,
    }
    if arguments.glosses is not None:
        meta["vocabulary"] = "offline-glosses " + gloss_dump_date(arguments.glosses)
    for language in languages:
        rows = found[language]
        if arguments.glosses is not None:
            vocabulary = gloss_vocabulary(arguments.glosses, language)
            restricted = {word: reading for word, reading in rows.items() if word in vocabulary}
            if language == "ja":
                for term in vocabulary:
                    if term not in restricted and is_kana(term) and hiragana(term) in rows:
                        restricted[term] = rows[hiragana(term)]
            rows = restricted
        if len(rows) < arguments.min_words:
            raise SystemExit(f"only {len(rows)} {language} readings; the source layout changed")
        path = arguments.out / f"{language}-phonetic.db"
        write_database(path, language, rows, meta)
        size = path.stat().st_size
        if size > arguments.max_bytes:
            raise SystemExit(f"{path}: {size} bytes is over the {arguments.max_bytes} byte limit")
        print(f"{language}-phonetic.db {len(rows):7} words {size:9} bytes")
    files = ", ".join(f"{language}-phonetic.db" for language in languages)
    notice = NOTICE.format(files=files, dump_date=arguments.dump_date, source_revision=arguments.source_revision)
    (arguments.out / NOTICE_NAME).write_text(notice, encoding="utf-8")
    return 0


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    commands = parser.add_subparsers(dest="command", required=True)
    filtering = commands.add_parser("filter")
    filtering.add_argument("--input", type=Path, action="append", required=True)
    filtering.add_argument("--out", type=Path, required=True)
    building = commands.add_parser("build")
    building.add_argument("--input", type=Path, required=True)
    building.add_argument("--out", type=Path, required=True)
    building.add_argument("--glosses", type=Path)
    building.add_argument("--lang", default=",".join(LANGUAGES))
    building.add_argument("--dump-date", default="unknown")
    building.add_argument("--source-revision", default="unknown")
    building.add_argument("--max-bytes", type=int, default=DEFAULT_MAX_BYTES)
    # 真实的 dump 每种语言都超过一万词，少于这个数说明 dump 的格式变了。离线测试只有几行，会把它调低。
    building.add_argument("--min-words", type=int, default=1000)
    arguments = parser.parse_args(argv)
    return filter_command(arguments) if arguments.command == "filter" else build_command(arguments)


if __name__ == "__main__":
    sys.exit(main())
