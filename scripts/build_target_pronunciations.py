#!/usr/bin/env python3
"""为非英文的候选释义行（法、日、西、俄、德、韩）构建离线读音表。

``en-phonetic.db`` 读英文释义行；这里给其余六种释义语言各建一张同样的表，来源与释义本身相同：英文维基词典，经 Wiktextract 抽取、由 kaikki.org 发布。离线释义（``build_offline_glosses.py``）取的是英语词条的译文表，译文行不带读音；读音取的是同一版维基词典里目标语言词条自己的读音，kaikki.org 按语言各发一个 JSONL dump（``kaikki.org-dictionary-French.jsonl.gz`` 等），格式与离线释义读的英语 dump 相同。

读音是这门语言的学习者被教的那种读法：

- 法、西、德、俄、韩用 ``sounds[].ipa``，按维基词典原样保留，连同斜线或方括号。斜线的音位标音优先于方括号的语音标音（西语两种都有，俄语和韩语只有方括号），不带地区标签的和标了标准音地区（西班牙、德国、首尔……）的优先于其他地区的。法语例外：dump 把标准的法国读音标成 ``France`` 或 ``Europe``，南部变体反而不带标签，所以法语里标了标准音地区的排在不带标签的前面（细节见 ``TAGGED_STANDARD_FIRST``）。只在联诵时出现的读法（``/tuz‿/``）和只标了一部分的（``[aːt͡s-]``）不用。德语的斜线是维基词典的音位层（``/ˈvasər/``、``/ˈhaːbən/``），学习者在词典里看到的实际读音（``[ˈvasɐ]``、``[ˈhaːbn̩]``）多半只在 ``sounds[].other`` 里或只标了词尾，这里不取它们：音位层是每个词都有、写法一致的那一层。
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
    phonetic = 1 if str(sound.get("ipa", "")).startswith("[") else 0
    if language in TAGGED_STANDARD_FIRST:
        region = 0 if standard else 1 if not regional else 2
        return phonetic * 3 + region
    region = 0 if not regional else 1 if standard else 2
    return region * 2 + phonetic


def entry_reading(entry: dict, language: str) -> str | None:
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

Changes: only the IPA transcriptions (French, Spanish, German, Russian, Korean) and the Hepburn romanization (Japanese) of each word were kept, one per word; archaic, dialectal and similar transcriptions, transcriptions holding more than one variant, liaison-only forms and partial transcriptions were removed; inflected forms were removed and only the words used by the offline candidate glosses were kept; the result was converted to SQLite.

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
