#!/usr/bin/env python3
"""Order Shuangpin whole-sentence candidates by lattice score.

The shared lattice already scores a Google-Pinyin fallback against the same
dictionary graph for Quanpin.  Shuangpin used the same decoder but always
moved the fallback ahead of a stronger lattice sentence after merging.  Wire
the existing ``WholeSentenceComparison`` result into that path so both
schemes use the same score arbitration while retaining the exact dictionary
boundary and candidate order.
"""

from pathlib import Path


def replace_once(path: Path, before: str, after: str, marker: str) -> None:
    text = path.read_text(encoding="utf-8")
    if marker in text:
        return
    count = text.count(before)
    if count != 1:
        raise RuntimeError(f"Engine overlay expected one match in {path}, found {count}")
    path.write_text(text.replace(before, after, 1), encoding="utf-8")


def apply(root: Path) -> None:
    source = root / "shuangpin/shuangpin_dictionary.cpp"
    before = """        quanpin::merge_lattice_candidates(candidate_list, quanpin_segments,
                                          quanpin::make_lattice_db_lookup(quanpin_db_, quanpin_statement_cache_,
                                                                          lattice_options.span_limit),
                                          pinyin_sequence, lattice_options);
        if (!google_sentence.empty())
"""
    after = """        quanpin::WholeSentenceComparison sentences;
        quanpin::merge_lattice_candidates(candidate_list, quanpin_segments,
                                          quanpin::make_lattice_db_lookup(quanpin_db_, quanpin_statement_cache_,
                                                                          lattice_options.span_limit),
                                          pinyin_sequence, lattice_options, google_sentence, &sentences);
        if (!google_sentence.empty() &&
            !sentences.lattice_outranks_fallback(lattice_options.fallback_margin))
"""
    replace_once(
        source,
        before,
        after,
        "lattice_options, google_sentence, &sentences);",
    )


if __name__ == "__main__":
    import sys

    apply(Path(sys.argv[1]))
