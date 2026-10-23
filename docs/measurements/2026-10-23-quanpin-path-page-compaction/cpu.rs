//! 普通分配器下的完整交替线程 CPU 矩阵；路径由同目录 run_cpu.py 注入。
use msime_engine::{CandidateSource, WordItem};
use std::hint::black_box;
use std::mem::size_of;

mod types {
    pub use msime_engine::{CandidateSource, SchemeType, WordItem};
}
mod dictionary {
    #[derive(Clone, Debug, PartialEq)]
    pub struct DictRow {
        pub key: String,
        pub value: String,
        pub weight: i64,
    }
}

trait Page: Clone {
    fn store(key: &str, rows: &[WordItem]) -> Self;
    fn copy(&self, key: &str) -> Vec<WordItem>;
    fn append(&self, key: &str, result: &mut Vec<WordItem>);
}
impl Page for Vec<WordItem> {
    fn store(_: &str, rows: &[WordItem]) -> Self {
        rows.to_vec()
    }
    fn copy(&self, _: &str) -> Vec<WordItem> {
        self.clone()
    }
    fn append(&self, _: &str, result: &mut Vec<WordItem>) {
        result.reserve(self.len());
        result.extend(self.iter().cloned());
    }
}
impl Page for path_page::PathPage {
    fn store(key: &str, rows: &[WordItem]) -> Self {
        Self::from_dictionary(key, rows)
    }
    fn copy(&self, key: &str) -> Vec<WordItem> {
        self.copy_rows(key)
    }
    fn append(&self, key: &str, result: &mut Vec<WordItem>) {
        self.append_to(key, result)
    }
}

fn cpu_ns() -> u64 {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // 仅测量当前线程的 CPU，宿主竞争造成的等待另由端到端实验记录。
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time) },
        0
    );
    time.tv_sec as u64 * 1_000_000_000 + time.tv_nsec as u64
}

fn run<P: Page>(mode: &str, keys: &[String], rows: &[Vec<WordItem>], repeats: usize) -> u64 {
    let mut hot = cache::FifoCache::<String, P>::new(128);
    if mode != "cold" {
        hot.insert(keys[0].clone(), P::store(&keys[0], &rows[0]));
    }
    let start = cpu_ns();
    for _ in 0..repeats {
        if mode == "cold" {
            let mut cold = cache::FifoCache::<String, P>::new(128);
            // 先填满，再以另外 128 键淘汰，再访问原键；包含构建与析构。
            for index in 0..384 {
                let at = index % 256;
                cold.insert(keys[at].clone(), P::store(&keys[at], black_box(&rows[at])));
            }
            black_box(&cold);
        } else {
            for _ in 0..384 {
                let page = hot.get_ref_by(black_box(keys[0].as_str())).unwrap();
                let output = if mode == "copy" {
                    page.copy(&keys[0])
                } else {
                    let mut output = vec![WordItem::new(
                        "seed",
                        "合成种子",
                        1,
                        CandidateSource::Database,
                        "seed",
                    )];
                    page.append(&keys[0], &mut output);
                    output
                };
                black_box(output);
            }
        }
    }
    cpu_ns() - start
}

fn equal(actual: &Vec<WordItem>, expected: &Vec<WordItem>) {
    assert_eq!(actual, expected);
    assert_eq!(actual.capacity(), expected.capacity());
    for (a, b) in actual.iter().zip(expected) {
        for (a, b) in [
            (&a.pinyin, &b.pinyin),
            (&a.canonical_pinyin, &b.canonical_pinyin),
            (&a.word, &b.word),
            (&a.corrected_from, &b.corrected_from),
        ] {
            assert_eq!(a.capacity(), b.capacity());
        }
        assert_eq!(a.sentence_words.capacity(), b.sentence_words.capacity());
    }
}

fn main() {
    eprintln!(
        "sizes: WordItem={} DictRow={} PathPage={} Vec={}",
        size_of::<WordItem>(),
        size_of::<dictionary::DictRow>(),
        size_of::<path_page::PathPage>(),
        size_of::<Vec<WordItem>>()
    );
    println!("rows,key_bytes,matching,mode,pair,order,repeats,old_ns,new_ns,old_page_bytes,new_page_bytes");
    for count in [0, 1, 2, 3, 4, 32, 128] {
        for key_bytes in [8, 64] {
            for matching in [true, false] {
                let keys: Vec<_> = (0..256)
                    .map(|index| format!("{:0width$}", index, width = key_bytes))
                    .collect();
                let rows: Vec<Vec<_>> = keys
                    .iter()
                    .map(|key| {
                        (0..count)
                            .map(|index| {
                                WordItem::new(
                                    if matching {
                                        key.as_str()
                                    } else {
                                        "different-reading"
                                    },
                                    format!("合成词{index}😀"),
                                    1000 - index as i64,
                                    CandidateSource::Database,
                                    "ni",
                                )
                            })
                            .collect()
                    })
                    .collect();
                let page = path_page::PathPage::store(&keys[0], &rows[0]);
                equal(&page.copy(&keys[0]), &rows[0].clone());
                let mut old_append = vec![WordItem::default()];
                let mut new_append = old_append.clone();
                rows[0].append(&keys[0], &mut old_append);
                page.append(&keys[0], &mut new_append);
                equal(&new_append, &old_append);
                let old_bytes = rows[0].len() * size_of::<WordItem>()
                    + rows[0]
                        .iter()
                        .map(|r| r.pinyin.len() + r.canonical_pinyin.len() + r.word.len())
                        .sum::<usize>();
                let new_bytes = page.retained_bytes();
                for mode in ["cold", "copy", "append"] {
                    let mut repeats = 1;
                    while run::<Vec<WordItem>>(mode, &keys, &rows, repeats) < 3_000_000 {
                        repeats *= 2;
                    }
                    for pair in 0..21 {
                        let (old, new) = if pair % 2 == 0 {
                            let old = run::<Vec<WordItem>>(mode, &keys, &rows, repeats);
                            (old, run::<path_page::PathPage>(mode, &keys, &rows, repeats))
                        } else {
                            let new = run::<path_page::PathPage>(mode, &keys, &rows, repeats);
                            (run::<Vec<WordItem>>(mode, &keys, &rows, repeats), new)
                        };
                        println!("{count},{key_bytes},{matching},{mode},{pair},{},{repeats},{old},{new},{old_bytes},{new_bytes}", if pair % 2 == 0 { "AB" } else { "BA" });
                    }
                }
            }
        }
    }
}
