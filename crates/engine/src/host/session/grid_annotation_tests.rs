// This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of the MPL was not distributed with this file, You can obtain one at http://mozilla.org/MPL/2.0/.

use super::*;
use crate::host::tests::helpcode_fixture;
use crate::nine_key_shuangpin::{
    resolve, resolve_eighteen_key_shuangpin, resolve_fourteen_key_shuangpin,
};
use crate::types::{NineKeyShuangpinKind, ShuangpinProfileKind};

#[test]
fn grid_annotations_follow_pinyin_display_rules_for_dictionary_and_generated_words() {
    let layouts = [
        (None, 0, "cangjie"),
        (None, 1, "chjp"),
        (Some(KeyGrid::NineKey), 0, "2264543"),
        (Some(KeyGrid::FourteenKey), 0, "cangjie"),
        (Some(KeyGrid::EighteenKey), 0, "cangjie"),
        (
            Some(KeyGrid::NineKeyShuangpin(
                resolve(NineKeyShuangpinKind::Migala3, None).unwrap(),
            )),
            0,
            "2554",
        ),
        (
            Some(KeyGrid::FourteenKeyShuangpin(
                resolve_fourteen_key_shuangpin(ShuangpinProfileKind::Xiaohe, None).unwrap(),
            )),
            1,
            "chjp",
        ),
        (
            Some(KeyGrid::EighteenKeyShuangpin(
                resolve_eighteen_key_shuangpin(ShuangpinProfileKind::Xiaohe, None).unwrap(),
            )),
            1,
            "chjp",
        ),
    ];
    for generated in [false, true] {
        for missing_code in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let extra = format!(
                "CREATE TABLE tbl_1_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_c VALUES('cang','c','仓',100);
CREATE TABLE tbl_1_j(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_j VALUES('jie','j','颉',100);
CREATE TABLE tbl_2_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);{}",
                if generated {
                    ""
                } else {
                    "INSERT INTO tbl_2_c VALUES('cang''jie','cj','仓颉',1000);"
                }
            );
            let mut options = helpcode_fixture(
                root.path(),
                &extra,
                if missing_code {
                    "仓=re\n"
                } else {
                    "仓=re\n颉=yj\n"
                },
            );
            for (grid, scheme, keys) in layouts {
                options.scheme = scheme;
                for enabled in [false, true] {
                    options.helpcode = enabled;
                    let mut previous_candidates = None;
                    for visible in [false, true] {
                        options.show_helpcode = visible;
                        let mut session = Session::new(&options).unwrap();
                        session.set_key_grid(grid).unwrap();
                        for (offset, letter) in keys.bytes().enumerate() {
                            if grid.is_none_or(|grid| grid.uses_digits()) {
                                session.character(letter, false).unwrap();
                            } else {
                                session.grid_key(letter).unwrap();
                            }
                            if matches!(grid, Some(KeyGrid::NineKeyShuangpin(_))) && offset == 1 {
                                let index = session
                                    .snapshot()
                                    .unwrap()
                                    .nine_key_spellings
                                    .iter()
                                    .position(|reading| reading == "cang")
                                    .unwrap();
                                session.choose_nine_key_spelling(index).unwrap();
                            }
                        }
                        let view = session.snapshot().unwrap();
                        let index = view
                            .candidates
                            .iter()
                            .position(|word| word == "仓颉")
                            .unwrap_or_else(|| {
                                panic!("{grid:?} scheme={scheme}: {:?}", view.candidates)
                            });
                        assert_eq!(
                            view.candidate_sources[index],
                            if generated {
                                CandidateSource::Generated
                            } else {
                                CandidateSource::Database
                            } as u8
                        );
                        let expected = if !enabled || !visible || missing_code {
                            ""
                        } else if scheme == 0 {
                            "(RY)"
                        } else {
                            "(rY)"
                        };
                        assert_eq!(
                            view.candidate_annotations[index], expected,
                            "{grid:?} generated={generated} enabled={enabled} visible={visible}"
                        );
                        assert_eq!(
                            session
                                .inner
                                .snapshot_for_host(visible)
                                .candidate_annotations[index],
                            expected,
                            "核心快照不依赖 host 补码"
                        );
                        if let Some(previous) = previous_candidates.replace(view.candidates) {
                            assert_eq!(
                                previous_candidates.as_ref().unwrap(),
                                &previous,
                                "显示开关不改变候选顺序"
                            );
                        }
                    }
                }
            }
        }
    }
}
