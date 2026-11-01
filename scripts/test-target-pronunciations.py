#!/usr/bin/env python3
"""build_target_pronunciations.py 的离线检查：过滤留下什么、每个词取哪个读音、写出的数据库。

词条照抄 2026-10-25 那一周 kaikki.org dump（法、日、西、俄、德、韩）的形状，只留构建要读的字段和几个必须忽略的字段。全部在临时文件上运行，不下载任何东西，所以 --quick 离线也能跑。
"""
import gzip
import json
import sqlite3
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.dont_write_bytecode = True
import build_target_pronunciations as generator  # noqa: E402

failures = []


def check(condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def entry(language, word, pos="noun", sounds=(), forms=(), senses=({"glosses": ["x"]},), **extra):
    return {"word": word, "lang_code": language, "lang": "x", "pos": pos, "sounds": list(sounds), "forms": list(forms), "senses": list(senses), **extra}


AUDIO = {"audio": "LL-Q150 (fra)-x.wav", "ogg_url": "https://upload.wikimedia.org/x.ogg", "tags": ["France"]}
DUMP = [
    # 音位标音胜过方括号的语音标音；音频行被过滤掉。
    entry("fr", "chat", sounds=[{"ipa": "[ʃa]"}, {"ipa": "/ʃa/"}, AUDIO]),
    # 名词词条的读音胜过 dump 里排在前面的动词词条。
    entry("fr", "est", pos="verb", sounds=[{"ipa": "/ɛ/"}]),
    entry("fr", "est", pos="noun", sounds=[{"ipa": "/ɛst/"}]),
    # 屈折形式有自己的 IPA，但没有释义写它。
    entry("fr", "chats", sounds=[{"ipa": "/ʃa/"}], senses=[{"form_of": [{"word": "chat"}], "tags": ["form-of", "plural"]}]),
    # 一格两种读法、一条方言读音，后面才是不带标签的。
    entry("fr", "pomme de terre", sounds=[{"ipa": "/pɔm də tɛʁ/, /pɔm t‿tɛʁ/"}, {"ipa": "/pɔm dœ tɛʁ/", "tags": ["dialectal"]}, {"ipa": "/pɔm də tɛʁ/"}]),
    # 不在任何释义里：只收释义用到的词时不收。
    entry("fr", "chien", sounds=[{"ipa": "/ʃjɛ̃/"}]),
    # 罗马字页不是释义会教的词。
    entry("fr", "Chat", pos="romanization", sounds=[{"ipa": "/tʃat/"}]),
    # 法语 dump 把标准读音标成 `Europe` 或 `France`，不带标签的反而是南部变体。
    entry("fr", "sept", pos="num", sounds=[{"ipa": "/sɛt/", "tags": ["Europe"]}, {"ipa": "/sɛtə/"}, {"ipa": "/saɛ̯t/", "tags": ["Canada"]}, {"ipa": "/sɛ/", "tags": ["archaic"]}]),
    entry("fr", "France", pos="name", sounds=[{"ipa": "/fʁɑ̃s/", "tags": ["France"]}, {"ipa": "/fʁãⁿsə/"}, {"ipa": "/fʁãs/", "tags": ["Quebec"]}]),
    # 只在联诵时出现的读法不用；全是联诵读法的词没有读音。
    entry("fr", "tous", pos="pron", sounds=[{"ipa": "/tuz‿/"}, {"ipa": "/tus/"}, {"ipa": "[tʊs]", "tags": ["Quebec"]}, {"ipa": "/tu/"}]),
    entry("fr", "cet", pos="det", sounds=[{"ipa": "/sɛ.t‿/"}, {"ipa": "/se.t‿/"}]),
    # 标 `France` 的方括号窄式记音不胜过不带标签的斜线标音。
    entry("fr", "bonjour", pos="intj", sounds=[{"ipa": "/bɔ̃.ʒuʁ/"}, {"ipa": "[bõ.ʒuə̯ʁ]", "tags": ["France"]}]),
    # 标 `France`、`Southern` 的是法国南部变体，`Paris` 标的是巴黎口音，都不算标准音。
    entry("fr", "jaune", pos="adj", sounds=[{"ipa": "/ʒon/"}, {"ipa": "/ʒɔn/", "tags": ["France", "Southern"]}]),
    entry("fr", "ce", pos="det", sounds=[{"ipa": "/sə/"}, {"ipa": "/sø/", "tags": ["Paris"]}]),
    # 西班牙读音在前，与释义偏向西班牙用法一致。
    entry("es", "cielo", sounds=[{"ipa": "/ˈθjelo/", "tags": ["Spain"]}, {"ipa": "/ˈsjelo/", "tags": ["Latin-America"]}]),
    entry("es", "gato", sounds=[{"ipa": "/ˈɡato/"}, {"ipa": "[ˈɡa.t̪o]"}]),
    # 俄语只有方括号标音；键是不带重音符的词头。
    entry("ru", "кошка", sounds=[{"ipa": "[ˈkoʂkə]"}, {"tags": ["Moscow"], "audio": "x.ogg"}]),
    # 韩语读罗马字，从维基词典的发音谚文（`sounds[].other`）生成，窄式 IPA 不用。
    entry("ko", "고양이", sounds=[{"ipa": "[ko̞ja̠ŋi]", "tags": ["SK-Standard", "Seoul"]}, {"other": "[고양이]"}]),
    # 紧音化不写；同一个词的第二种读法（`[일따]`）不用。
    entry("ko", "학교", sounds=[{"ipa": "[ha̠k̚k͈jo]", "tags": ["SK-Standard", "Seoul"]}, {"other": "[학꾜]"}]),
    entry("ko", "읽다", pos="verb", sounds=[{"ipa": "[ik̚t͈a̠]", "tags": ["SK-Standard", "Seoul"]}, {"other": "[익따]"}, {"ipa": "[iɭt͈a̠]", "tags": ["SK-Standard", "Seoul"]}, {"other": "[일따]"}]),
    # 可选长音记号去掉；谓词照常送气，体词里的 ㅎ 照写，专名首字母大写。
    entry("ko", "좋다", pos="adj", sounds=[{"ipa": "[ˈt͡ɕo̞(ː)tʰa̠]", "tags": ["SK-Standard", "Seoul"]}, {"other": "[조(ː)타]"}]),
    entry("ko", "묵호", pos="name", sounds=[{"ipa": "[mukʰo̞]", "tags": ["SK-Standard", "Seoul"]}, {"other": "[무코]"}]),
    # 元音按拼写写；维基词典写出的 ㅎ 弱化读法不算；收音移过来的紧音、外来词的口语紧音不写。
    entry("ko", "희망", sounds=[{"ipa": "[çima̠ŋ]", "tags": ["SK-Standard", "Seoul"]}, {"other": "[히망]"}]),
    entry("ko", "전화", sounds=[{"ipa": "[ˈt͡ɕʌ̹(ː)nwa̠]", "tags": ["SK-Standard", "Seoul"]}, {"other": "[저(ː)놔]"}]),
    entry("ko", "없이", pos="adv", sounds=[{"ipa": "[ʌ̹ːp̚ɕ͈i]", "tags": ["SK-Standard", "Seoul"]}, {"other": "[업ː씨]"}]),
    entry("ko", "카페", sounds=[{"ipa": "[k͈a̠pʰe̞]", "tags": ["SK-Standard", "Seoul"]}, {"other": "[까페]"}]),
    # 维基词典没写发音谚文时按拼写推出发音；词与词之间的同化不写。
    entry("ko", "개", sounds=[{"ipa": "[kɛ(ː)]", "tags": ["SK-Standard", "Seoul"]}, {"ipa": "[ke̞(ː)]", "tags": ["SK-Standard", "Seoul"]}]),
    entry("ko", "누워서 떡 먹기", pos="proverb", sounds=[{"other": "[누워서 떵 먹끼]"}]),
    # 词头不是谚文的词条没有罗马字。
    entry("ko", "學校", sounds=[{"ipa": "[ha̠k̚k͈jo]", "tags": ["SK-Standard", "Seoul"]}, {"other": "[학꾜]"}]),
    # 德语名词保留大写；格里留着注释的整格不用；只标了一部分的不用；不带标签的胜过标 `Germany` 的变体。
    entry("de", "Katze", sounds=[{"ipa": "/ˈkatsə/"}]),
    entry("de", "Kater", sounds=[{"ipa": "/ˈkaːtɐ/ (Austria)"}]),
    entry("de", "Arzneimittel", sounds=[{"ipa": "[aːt͡s-]", "tags": ["Germany", "Northern"]}, {"ipa": "[aʁt͡sˈnaɪ̯ˌmɪtl̩]"}]),
    entry("de", "Chile", pos="name", sounds=[{"ipa": "/ˈt͡ʃiːlə/"}, {"ipa": "/ˈçiːlə/", "tags": ["Germany"]}]),
    # 德语的方括号语音标音胜过斜线的音位标音，只标了词尾的方括号不用；口语缩读不用。
    entry("de", "haben", pos="verb", sounds=[{"ipa": "/ˈhaːbən/"}, {"ipa": "[ˈhaː.bən]"}, {"ipa": "[-bn̩]"}, {"ipa": "/ham/"}, {"ipa": "[ham]"}]),
    entry("de", "er", pos="pron", sounds=[{"ipa": "/eːr/", "tags": ["standard"]}, {"ipa": "[ʔeːɐ̯]", "tags": ["standard"]}, {"ipa": "/ɐ/", "tags": ["colloquial"]}]),
    entry("de", "mal", pos="adv", sounds=[{"ipa": "/ma/", "tags": ["colloquial"]}, {"ipa": "/maːl/", "tags": ["standard"]}]),
    # 地区先比：只有标了其他地区的方括号时给不带标签的斜线；只写在 `other` 里的方括号不取。
    entry("de", "Samstag", sounds=[{"ipa": "/ˈzamstaːk/", "tags": ["standard"]}, {"ipa": "/ˈzamstax/"}, {"ipa": "[sɑmsd̥ɑɡ̥]", "tags": ["Austria", "Bavaria", "Southern-Germany"]}]),
    entry("de", "Wasser", sounds=[{"ipa": "/ˈvasər/"}, {"other": "[ˈva.sɐ]"}, {"other": "[ˈva.səʁ]"}]),
    # 日语读 romanization 形式的赫本式罗马字，从不用窄式 IPA；汉字词条的假名写法也读片假名释义。
    entry("ja", "林檎", sounds=[{"ipa": "[ɾʲiŋɡo̞]"}, {"other": "りんご"}], forms=[{"form": "林檎", "tags": ["canonical"]}, {"form": "ringo", "tags": ["romanization"]}]),
    entry("ja", "猫", sounds=[{"ipa": "[ne̞ko̞]"}, {"other": "ねこ"}], forms=[{"form": "neko", "tags": ["romanization"]}]),
    entry("ja", "リンゴ", sounds=[], forms=[], senses=[{"alt_of": [{"word": "林檎"}], "tags": ["alt-of"]}]),
    # 同一词性的同形词条：带假名并标了声调的胜过 dump 里排在前面、只带假名的（`日` 是 `hi` 不是 `nichi`），带假名的又胜过不带的罕用读法（`nekoma`）。
    entry("ja", "日", sounds=[{"ipa": "[ɲ̟it͡ɕi]"}, {"other": "にち"}], forms=[{"form": "nichi", "tags": ["romanization"]}]),
    entry("ja", "日", sounds=[{"ipa": "[çi]"}, {"other": "ひ"}, {"other": "[híꜜì]"}, {"other": "[hî]", "tags": ["Kagoshima"]}], forms=[{"form": "hi", "tags": ["romanization"]}]),
    entry("ja", "猫", sounds=[{"ipa": "[ne̞ko̞ma̠]"}], forms=[{"form": "nekoma", "tags": ["romanization"]}]),
    # 含假名的 romanization 不是罗马字。
    entry("ja", "空", sounds=[{"other": "そら"}], forms=[{"form": "そら", "tags": ["romanization"]}]),
    # 英语和其他语言不归这个构建管。
    entry("en", "cat", sounds=[{"ipa": "/kæt/"}]),
    {"word": "", "lang_code": "fr", "sounds": [{"ipa": "/x/"}]},
]

GLOSSES = {
    "fr": ["chat, chatte; félin", "pomme de terre", "est; orient", "Chat", "sept", "France", "tous", "cet", "bonjour", "jaune", "ce"],
    "es": ["gato, gata", "cielo; día"],
    "ru": ["кошка, кот"],
    "de": ["Katze, Kater", "Arzneimittel", "Chile", "haben", "er", "mal", "Samstag", "Wasser"],
    "ko": ["고양이", "학교", "읽다", "좋다", "묵호", "희망", "전화", "없이", "카페", "개", "누워서 떡 먹기", "學校"],
    "ja": ["林檎, リンゴ", "家猫, 猫", "空, 大空", "日"],
}


def write_glosses(directory: Path) -> None:
    directory.mkdir(parents=True)
    for language, glosses in GLOSSES.items():
        with sqlite3.connect(directory / f"zh-{language}.db") as database:
            database.execute("CREATE TABLE zh_glosses(chinese TEXT PRIMARY KEY, gloss TEXT NOT NULL, source TEXT NOT NULL)")
            database.execute("CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT NOT NULL)")
            database.executemany("INSERT INTO zh_glosses VALUES (?, ?, 'x')", [(str(index), gloss) for index, gloss in enumerate(glosses)])
            database.executemany("INSERT INTO meta VALUES (?, ?)", [("target_language", language), ("dump_date", "2026-09-02")])


# 文化观光部告示第 2000-8 号《国语的罗马字表记法》正文里的例词，不带发音谚文，全靠从拼写推出发音。规范里写成专名的例词按专名（`name`）查，首字母大写。
RULE_EXAMPLES = {
    # 第 2 章：元音和辅音，收音的代表音。
    "구미": "Gumi", "영동": "Yeongdong", "백암": "Baegam", "옥천": "Okcheon", "합덕": "Hapdeok", "호법": "Hobeop", "월곶": "Wolgot",
    "벚꽃": "beotkkot", "한밭": "Hanbat", "구리": "Guri", "설악": "Seorak", "칠곡": "Chilgok", "임실": "Imsil", "울릉": "Ulleung",
    "대관령": "Daegwallyeong", "광희문": "Gwanghuimun",
    # 第 3 章第 1 项：鼻音化、流音化、腭化、送气；紧音化不写，体词里 ㄱ、ㄷ、ㅂ 后的 ㅎ 照写。
    "백마": "Baengma", "종로": "Jongno", "왕십리": "Wangsimni", "별내": "Byeollae", "신라": "Silla", "해돋이": "haedoji", "같이": "gachi",
    "굳히다": "guchida", "좋고": "joko", "놓다": "nota", "잡혀": "japyeo", "낳지": "nachi", "묵호": "Mukho", "집현전": "Jiphyeonjeon",
    "압구정": "Apgujeong", "낙동강": "Nakdonggang", "죽변": "Jukbyeon", "낙성대": "Nakseongdae", "합정": "Hapjeong", "팔당": "Paldang",
    "샛별": "saetbyeol", "울산": "Ulsan",
    # 第 3 章第 2 项：音节界线会读错的地方加连字符。
    "중앙": "Jung-ang", "반구대": "Ban-gudae", "세운": "Se-un", "해운대": "Hae-undae",
}
# 例词里的谓词：送气照写，不按体词的例外。
RULE_EXAMPLE_PREDICATES = {"굳히다", "좋고", "놓다", "잡혀", "낳지"}
# 规范只用发音谚文能写对的例词（ㄴ 添加推不出来），给它们维基词典写的发音。
PRONOUNCED_EXAMPLES = {"학여울": ("[항녀울]", "Hangnyeoul"), "알약": ("[알략]", "allyak")}


def check_revised_romanization() -> None:
    for word, expected in RULE_EXAMPLES.items():
        pos = "name" if expected[:1].isupper() else "verb" if word in RULE_EXAMPLE_PREDICATES else "noun"
        reading = generator.revised_romanization(word, {"pos": pos, "sounds": []})
        check(reading == expected, f"{word} romanizes from its spelling as {expected!r}, not {reading!r}")
    for word, (pronunciation, expected) in PRONOUNCED_EXAMPLES.items():
        pos = "name" if expected[:1].isupper() else "noun"
        reading = generator.revised_romanization(word, {"pos": pos, "sounds": [{"other": pronunciation}]})
        check(reading == expected, f"{word} romanizes from {pronunciation} as {expected!r}, not {reading!r}")
    check(generator.revised_romanization("學校", {"pos": "noun", "sounds": [{"other": "[학꾜]"}]}) is None, "a headword that is not Hangul has no romanization")
    check(generator.revised_romanization("한국어", {"pos": "noun", "sounds": [{"other": "[한구거]", "tags": ["North-Korea"]}]}) == "han-gugeo", "a tagged pronunciation is not used; the spelling gives the same reading")


def table(path: Path, language: str) -> dict[str, str]:
    with sqlite3.connect(path) as database:
        return dict(database.execute(f"SELECT word, phonetic FROM {language}_phonetics"))


def main() -> int:
    check(generator.clean_ipa("/ʃa/") == "/ʃa/", "a slashed transcription is kept with its delimiters")
    check(generator.clean_ipa("[ˈkoʂkə]") == "[ˈkoʂkə]", "a bracketed transcription is kept")
    check(generator.clean_ipa("/ʃa/, /ʃat/") is None, "two variants in one cell are not one reading")
    check(generator.clean_ipa("/ˈkaːtɐ/ (Austria)") is None, "a note after the transcription rejects it")
    check(generator.clean_ipa("ʃa") is None, "a cell without delimiters is not a transcription")
    check(generator.clean_ipa("/ʃa3/") is None, "digits are not IPA")
    check(generator.clean_ipa("/ʃ(a/") is None, "unbalanced brackets reject the cell")
    check(generator.clean_romaji("ōkii") == "ōkii" and generator.clean_romaji("konnichi wa") == "konnichi wa", "Hepburn with macrons and spaces is romaji")
    check(generator.clean_romaji("そら") is None and generator.clean_romaji("") is None, "kana is not romaji")
    check(generator.hiragana("リンゴ") == "りんご" and generator.hiragana("コンピューター") == "こんぴゅーたー", "katakana folds to hiragana, the long-vowel mark kept")
    check_revised_romanization()

    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        dump = root / "dump.jsonl.gz"
        with gzip.open(dump, "wt", encoding="utf-8") as handle:
            for item in DUMP:
                handle.write(json.dumps(item, ensure_ascii=False) + "\n")
        filtered = root / "filtered.jsonl.gz"
        check(generator.main(["filter", "--input", str(dump), "--out", str(filtered)]) == 0, "filter runs")
        first = filtered.read_bytes()
        check(generator.main(["filter", "--input", str(dump), "--out", str(filtered)]) == 0 and filtered.read_bytes() == first, "the filtered dump is byte-identical across runs")
        kept = [json.loads(line) for line in gzip.open(filtered, "rt", encoding="utf-8")]
        check(all(item["lang_code"] in generator.LANGUAGES for item in kept), "only the six languages are kept")
        check(not any("audio" in sound or "ogg_url" in sound for item in kept for sound in item["sounds"]), "audio rows are dropped")
        check(not any(item["word"] == "リンゴ" for item in kept), "an entry without any reading is dropped")
        check(any(item["word"] == "chats" and item.get("form_of") for item in kept), "an inflected form is marked")
        check(all(set(item) <= {"word", "lang_code", "pos", "sounds", "romanizations", "form_of"} for item in kept), "only the fields the build reads are kept")

        glosses = root / "glosses"
        write_glosses(glosses)
        out = root / "out"
        arguments = ["build", "--input", str(filtered), "--out", str(out), "--glosses", str(glosses), "--min-words", "1",
                     "--dump-date", "2026-10-25", "--source-revision", "f9fe161b9dd1e898352c51bef7295e774bba2ef1"]
        check(generator.main(arguments) == 0, "build runs")
        french = table(out / "fr-phonetic.db", "fr")
        check(french.get("chat") == "/ʃa/", f"a phonemic transcription beats a phonetic one: {french.get('chat')!r}")
        check(french.get("est") == "/ɛst/", f"the noun entry beats an earlier verb entry: {french.get('est')!r}")
        check(french.get("pomme de terre") == "/pɔm də tɛʁ/", f"multi-variant and dialectal cells are skipped: {french.get('pomme de terre')!r}")
        check("chats" not in french and "chien" not in french, "inflected forms and words outside the glosses are left out")
        check("Chat" not in french, "a romanization page gives no reading")
        check(french.get("sept") == "/sɛt/" and french.get("France") == "/fʁɑ̃s/", f"a France or Europe tagged transcription beats the untagged Southern one: {french.get('sept')!r} {french.get('France')!r}")
        check(french.get("tous") == "/tus/" and "cet" not in french, f"liaison-only forms are not readings: {french.get('tous')!r} {french.get('cet')!r}")
        check(french.get("bonjour") == "/bɔ̃.ʒuʁ/", f"a France tagged narrow transcription does not beat the phonemic one: {french.get('bonjour')!r}")
        check(french.get("jaune") == "/ʒon/" and french.get("ce") == "/sə/", f"Southern France and Paris accent forms are not the standard: {french.get('jaune')!r} {french.get('ce')!r}")
        spanish = table(out / "es-phonetic.db", "es")
        check(spanish == {"gato": "/ˈɡato/", "cielo": "/ˈθjelo/"}, f"Spanish keeps the phonemic and the standard reading: {spanish}")
        check(table(out / "ru-phonetic.db", "ru") == {"кошка": "[ˈkoʂkə]"}, "Russian keeps its bracketed transcription")
        korean = table(out / "ko-phonetic.db", "ko")
        check(korean == {
            "고양이": "goyang-i", "학교": "hakgyo", "읽다": "ikda", "좋다": "jota", "묵호": "Mukho", "희망": "huimang",
            "전화": "jeonhwa", "없이": "eopsi", "카페": "kape", "개": "gae", "누워서 떡 먹기": "nuwoseo tteok meokgi",
        }, f"Korean reads in the Revised Romanization of its spelling and Wiktionary's Hangul pronunciation: {korean}")
        german = table(out / "de-phonetic.db", "de")
        check(german == {
            "Katze": "/ˈkatsə/", "Arzneimittel": "[aʁt͡sˈnaɪ̯ˌmɪtl̩]", "Chile": "/ˈt͡ʃiːlə/", "haben": "[ˈhaː.bən]", "er": "[ʔeːɐ̯]",
            "mal": "/maːl/", "Samstag": "/ˈzamstaːk/", "Wasser": "/ˈvasər/",
        }, f"German keeps the capital, drops noted, partial and colloquial cells, and prefers the untagged reading, then the bracketed one: {german}")
        japanese = table(out / "ja-phonetic.db", "ja")
        check(japanese == {"林檎": "ringo", "猫": "neko", "リンゴ": "ringo", "日": "hi"}, f"Japanese reads in Hepburn, katakana through the kanji entry's kana, and prefers the accented common reading: {japanese}")
        with sqlite3.connect(out / "ja-phonetic.db") as database:
            meta = dict(database.execute("SELECT key, value FROM meta"))
            version = database.execute("PRAGMA user_version").fetchone()[0]
        check(meta.get("kind") == "ja_phonetic" and meta.get("language") == "ja", "meta names the kind and language the bridge checks")
        check(meta.get("license") == "CC-BY-SA-4.0" and meta.get("vocabulary") == "offline-glosses 2026-09-02", f"meta records the licence and the gloss vocabulary: {meta}")
        check(version == generator.SCHEMA_VERSION, "user_version is the schema version")
        notice = (out / generator.NOTICE_NAME).read_text(encoding="utf-8")
        check("Creative Commons Attribution-ShareAlike 4.0" in notice and "Wiktionary contributors" in notice, "the NOTICE carries the attribution and the licence")
        check("Wiktextract dump date: 2026-10-25" in notice and "f9fe161b9dd1e898352c51bef7295e774bba2ef1" in notice, "the NOTICE names the dump")
        digests = {path.name: path.read_bytes() for path in sorted(out.iterdir())}
        check(generator.main(arguments) == 0, "build runs again")
        check(digests == {path.name: path.read_bytes() for path in sorted(out.iterdir())}, "the same input gives the same bytes")
        unrestricted = root / "unrestricted"
        check(generator.main(["build", "--input", str(filtered), "--out", str(unrestricted), "--min-words", "1"]) == 0, "build without glosses runs")
        everything = table(unrestricted / "fr-phonetic.db", "fr")
        check("chien" in everything and "chats" not in everything, "without --glosses every lemma is kept, still without inflected forms")
        check(table(unrestricted / "ja-phonetic.db", "ja").get("りんご") == "ringo", "without --glosses the kana spellings are their own keys")
        try:
            generator.main(["build", "--input", str(filtered), "--out", str(root / "few"), "--glosses", str(glosses)])
            check(False, "a dump with too few readings must fail the build")
        except SystemExit as error:
            check("source layout changed" in str(error), f"too few readings fail with the layout message: {error}")
    return report()


def report() -> int:
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        return 1
    print("target pronunciations: filter, reading choice and databases OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
