use std::hint::black_box;
use std::time::Instant;
#[rustfmt::skip]
#[path="__CACHE__"] mod cache;
mod types {
    #[derive(Clone)]
    pub struct WordItem(pub u64);
}
mod path_cache {
    use crate::{cache::FifoCache, types::WordItem};
    use compact_str::CompactString;
    pub struct PathCache(FifoCache<CompactString, Vec<WordItem>>);
    impl PathCache {
        pub fn new(n: usize) -> Self {
            Self(FifoCache::new(n))
        }
        pub fn insert_ref(&mut self, key: &str, rows: Vec<WordItem>) {
            self.0.insert(CompactString::new(key), rows);
        }
        pub fn get_ref_by(&self, key: &str) -> Option<&Vec<WordItem>> {
            self.0.get_ref_by(key)
        }
    }
}
use cache::FifoCache;
use path_cache::PathCache;
fn now() -> u64 {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time) },
        0
    );
    time.tv_sec as u64 * 1_000_000_000 + time.tv_nsec as u64
}
fn run(new: bool, keys: &[String], rows: usize, mode: usize, iterations: usize) -> u64 {
    let capacity = if keys.len() > 128 { 128 } else { 1 };
    let mut old = FifoCache::new(capacity);
    let mut candidate = PathCache::new(capacity);
    let page = vec![types::WordItem(17); rows];
    for key in keys.iter().take(capacity) {
        if new {
            candidate.insert_ref(key, page.clone());
        } else {
            old.insert(key.to_owned(), page.clone());
        }
    }
    let start = now();
    for index in 0..iterations {
        let key = black_box(
            &keys[if mode == 2 {
                0
            } else if mode == 1 {
                index % capacity
            } else {
                index % keys.len()
            }],
        );
        if mode == 1 {
            if new {
                black_box(candidate.get_ref_by(key));
            } else {
                black_box(old.get_ref_by(key.as_str()));
            }
        } else if new {
            candidate.insert_ref(key, black_box(page.clone()));
        } else {
            old.insert(key.to_owned(), black_box(page.clone()));
        }
    }
    black_box((old, candidate));
    now() - start
}
fn main() {
    let wall = Instant::now();
    for length in [0usize, 1, 8, 23, 31, 32, 64] {
        let keys: Vec<String> = if length == 0 {
            vec![String::new()]
        } else if length == 1 {
            vec!["a".into(), "b".into()]
        } else {
            (0..256)
                .map(|n| format!("{n:03}{}", "x".repeat(length - 3)))
                .collect()
        };
        for rows in [0usize, 1, 32] {
            for mode in 0..3 {
                let mut ratios = Vec::new();
                for pair in 0..21 {
                    let (a, b) = if pair % 2 == 0 {
                        (
                            run(false, &keys, rows, mode, 20000),
                            run(true, &keys, rows, mode, 20000),
                        )
                    } else {
                        let b = run(true, &keys, rows, mode, 20000);
                        (run(false, &keys, rows, mode, 20000), b)
                    };
                    ratios.push(b as f64 / a as f64);
                }
                ratios.sort_by(f64::total_cmp);
                println!("{{\"length\":{length},\"rows\":{rows},\"mode\":{mode},\"ratio\":{},\"ratios\":{:?}}}",ratios[ratios.len()/2],ratios);
            }
        }
    }
    eprintln!("wall_seconds={}", wall.elapsed().as_secs_f64());
}
