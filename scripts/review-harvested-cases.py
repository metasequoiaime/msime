#!/usr/bin/env python3
"""Screen harvested conversion failures before they become evaluation cases.

The harvester keeps any sentence whose pinyin decodes to something else. That is necessary but not
sufficient: `resources/eval/README.md` excludes sentences whose pinyin has two equally natural
readings, because "评测集不能既诚实又有歧义" — a case whose gold is one of two defensible answers
measures the grader, not the decoder. Deciding that is a judgement, not a filter.

Four questions per case, asked together over the same state because none needs another's answer:

  intended     Choice over the readings the decoder actually produced. If this disagrees with the
               original, the case is not usable: either the corpus sentence is odd, or the decoder's
               answer is defensible and the "failure" is not one.
  well_formed  Noul on whether the chosen reading is connected Chinese someone would type, rather
               than a page title, a run of keywords or a fragment. Web corpora are full of those
               and they make poor evaluation cases whatever the decoder does with them.
  well_written Noul on whether that reading is correct Chinese at all. A corpus sentence carrying
               its own typo makes a case whose gold is wrong, and those are worse than useless:
               人多为患 penalises exactly the scorer good enough to propose 人满为患.
  ambiguous    Noul on whether more than one reading is equally natural here. This is the README's
               own exclusion rule, asked directly. Recorded, but no longer a veto — see below.

A case is proposed when the choice lands on the original and that reading is a correctly written
sentence. Everything else is written out with the reason attached, for a person to scan. The script proposes;
it does not decide. Nothing here is a gate.

Input is convert_eval's `--dump` JSONL, which carries the real candidate list, the gold and the
context. Text sent to the API is licensed corpus, never user input.

usage: TYPESAFE_API_KEY=... scripts/review-harvested-cases.py <dump.jsonl> <out-prefix>
"""

import json
import os
import sys
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor

ENDPOINT = "https://api.typesafe.ai/v1/systemone"
MODEL = "jev-latest"
MAX_RESPONSE_BYTES = 1 << 20

# Thresholds are starting points to evaluate on this data, not settled policy.
#
# The choice is the primary gate: it is the question with a checkable answer, and its confidence is
# a distribution concentration rather than a guess about a guess.
#
# Ambiguity was a veto and is no longer one. On 1200 cases harvested from C4 it rejected 436 whose
# choice had landed on the original — every single one of them — and it spread its scores over
# 0.65 to 0.87 with no visible relation to whether the case was actually ambiguous: 吃得白白胖胖
# scored 0.72 although 吃的白白胖胖 is not a sentence at all. That is the same failure the 0.40
# threshold had before it, one band higher. The number is still recorded in the flagged file, so a
# person can look for a band where it means something; nothing decides on it.
#
# What those 436 cases actually needed are the two questions that replaced it. C4 is web text, so a
# large share of what survives harvesting is page titles and keyword runs — 黄页三门峡分站,
# 南岗洗车店, 资江天气. They are not ambiguous, they are not sentences, and a reranking evaluation
# built on them measures how well a model ranks navigation furniture.
#
# The second came from reading 30 accepted cases by hand: three carried the corpus's own errors —
# 人多为患 for 人满为患, 济南是自然风景优美, 如何考出钢琴. Those are worse than noise, because the
# scorer good enough to propose the correct writing is the one they penalise.
MIN_CHOICE_CONFIDENCE = 0.60
MIN_WELL_FORMED = 0.50
MIN_WELL_WRITTEN = 0.50

KEY = os.environ.get("TYPESAFE_API_KEY", "")
if not KEY:
    sys.exit("TYPESAFE_API_KEY is not set")


def read_response_json(response):
    """Decode one bounded API response without allowing a hostile body to exhaust memory."""
    payload = response.read(MAX_RESPONSE_BYTES + 1)
    if len(payload) > MAX_RESPONSE_BYTES:
        raise ValueError("API response exceeds the byte limit")
    return json.loads(payload)


def ask(case):
    options, seen = [], set()
    for candidate in case["candidates"]:
        text = candidate["text"]
        if text not in seen:
            seen.add(text)
            options.append(text)
    if case["gold"] not in options:
        case["verdict"] = "gold-absent"
        return case
    # The decoder returns long tails; the readings worth comparing are the ones that answered the
    # same key, which is the length the gold is.
    width = len(case["gold"])
    options = [text for text in options if len(text) == width][:9]
    if case["gold"] not in options or len(options) < 2:
        case["verdict"] = "no-comparable-alternatives"
        return case

    body = {
        "state": {
            "pinyin_keystrokes": case["input"],
            "preceding_text": case.get("context", ""),
            "readings": options,
        },
        "model": MODEL,
        "questions": {
            "intended": {
                "type": "choice",
                "instructions": (
                    "A Chinese writer typed `pinyin_keystrokes` into an input method, "
                    "continuing after `preceding_text`. Every option spells exactly those "
                    "keystrokes and differs only in which characters were chosen. Which one "
                    "did the writer mean?"
                ),
                "criteria": {text: None for text in options},
            },
            "well_written": {
                "type": "noul",
                "instructions": (
                    "Is the option named by `intended` correct Chinese — free of typos, "
                    "wrong characters and broken grammar?"
                ),
                "criteria": {
                    "true": "It is written the way a careful writer would write it.",
                    "false": (
                        "It contains a miswritten character, a malformed set phrase, or a "
                        "grammatical error — 人多为患 where 人满为患 is meant, or a sentence "
                        "that does not parse."
                    ),
                },
            },
            "well_formed": {
                "type": "noul",
                "instructions": (
                    "Is the option named by `intended` a self-contained, naturally written "
                    "Chinese sentence or clause — the kind of thing a person types into a "
                    "message or a document?"
                ),
                "criteria": {
                    "true": "It reads as ordinary connected Chinese that someone would type.",
                    "false": (
                        "It is a web page title, a run of keywords, a site or product name, "
                        "or a fragment cut out of the middle of a sentence."
                    ),
                },
            },
            "ambiguous": {
                "type": "noul",
                "instructions": (
                    "Considering `readings` for `pinyin_keystrokes` after `preceding_text`: "
                    "is there more than one reading a competent writer could equally well have "
                    "intended here?"
                ),
                "criteria": {
                    "true": (
                        "At least two of the readings are equally natural in this context, so "
                        "which one was meant cannot be settled from the text alone."
                    ),
                    "false": (
                        "Exactly one reading is natural here; the others are wrong or clearly "
                        "less plausible."
                    ),
                },
            },
        },
    }

    request = urllib.request.Request(
        ENDPOINT,
        data=json.dumps(body).encode(),
        headers={"Authorization": f"Bearer {KEY}", "Content-Type": "application/json"},
    )
    for attempt in range(5):
        try:
            with urllib.request.urlopen(request, timeout=40) as response:
                answer = read_response_json(response)
            break
        except urllib.error.HTTPError as error:
            if error.code in (429, 500, 502, 503, 504) and attempt < 4:
                time.sleep(min(2**attempt, 8))
                continue
            case["verdict"] = f"error HTTP {error.code}"
            return case
        except Exception as error:  # noqa: BLE001
            if attempt < 4:
                time.sleep(min(2**attempt, 8))
                continue
            case["verdict"] = f"error {type(error).__name__}"
            return case
    else:
        case["verdict"] = "error retries exhausted"
        return case

    intended = answer["answers"]["intended"]
    ambiguous = answer["answers"]["ambiguous"]
    well_formed = answer["answers"]["well_formed"]
    well_written = answer["answers"]["well_written"]
    case["jev_choice"] = intended["choice"]
    case["jev_confidence"] = intended.get("confidence")
    case["jev_ambiguity"] = ambiguous.get("noul")
    case["jev_well_formed"] = well_formed.get("noul")
    case["jev_well_written"] = well_written.get("noul")
    case["usage"] = answer.get("usage", {})

    if case["jev_choice"] != case["gold"]:
        case["verdict"] = "disagrees-with-original"
    elif (case["jev_confidence"] or 0) < MIN_CHOICE_CONFIDENCE:
        case["verdict"] = "low-confidence"
    elif (case["jev_well_formed"] or 0) < MIN_WELL_FORMED:
        case["verdict"] = "not-a-sentence"
    elif (case["jev_well_written"] or 0) < MIN_WELL_WRITTEN:
        case["verdict"] = "corpus-error"
    else:
        case["verdict"] = "accept"
    return case


def row(case):
    syllables = len(case["gold"])
    return "\t".join(
        [
            case["id"],
            case["input"],
            case["gold"],
            str(syllables),
            "harvested",
            case.get("context", ""),
        ]
    )


def main():
    dump, prefix = sys.argv[1], sys.argv[2]
    cases = [json.loads(line) for line in open(dump) if line.strip()]
    print(f"{len(cases)} harvested cases", file=sys.stderr)

    with ThreadPoolExecutor(max_workers=8) as pool:
        cases = list(pool.map(ask, cases))

    accepted = [c for c in cases if c["verdict"] == "accept"]
    with open(f"{prefix}-accepted.tsv", "w") as handle:
        handle.write("# Jev 评审通过的收割用例，仍待人工确认后并入评测集。\n")
        handle.write("# 列：id / 拼音 / 金标准 / 字数 / 标签 / 上文\n")
        for case in accepted:
            handle.write(row(case) + "\n")
    with open(f"{prefix}-flagged.tsv", "w") as handle:
        handle.write("# 未通过，附理由。disagrees-with-original 往往说明引擎的答案也站得住；\n")
        handle.write("# not-a-sentence 多半是网页标题或关键词串，语料本身的问题，不是解码的。\n")
        handle.write("# 列：判定 / id / 拼音 / 金标准 / 评审所选 / 歧义 / 成句 / 无误 / 上文\n")
        for case in cases:
            if case["verdict"] == "accept":
                continue
            handle.write(
                "\t".join(
                    [
                        case["verdict"],
                        case["id"],
                        case["input"],
                        case["gold"],
                        case.get("jev_choice", ""),
                        f"{case.get('jev_ambiguity', -1):.2f}",
                        f"{case.get('jev_well_formed', -1):.2f}",
                        f"{case.get('jev_well_written', -1):.2f}",
                        case.get("context", ""),
                    ]
                )
                + "\n"
            )

    counts = {}
    for case in cases:
        counts[case["verdict"]] = counts.get(case["verdict"], 0) + 1
    tokens = sum(c.get("usage", {}).get("input_tokens", 0) for c in cases)
    for verdict, count in sorted(counts.items(), key=lambda item: -item[1]):
        print(f"  {verdict:28} {count}", file=sys.stderr)
    print(
        f"{len(accepted)} proposed -> {prefix}-accepted.tsv "
        f"({tokens} input tokens, ${tokens / 1e6 * 0.042:.4f})",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
