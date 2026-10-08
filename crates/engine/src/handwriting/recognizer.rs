//! Offline recognition of one handwritten character: the C++ `metasequoia::handwriting::Recognizer` (`handwriting.cpp`) and the bridge entry that fed it (`bridge.cpp` `handwriting_recognize`), over a Rust port of zinnia.
//!
//! The reference constructed a recognizer, and so re-mapped the model, on every call. Here recent model paths are mapped and parsed once while they stay in a bounded cache: host-api classifies each character cell of a written line separately, and re-parsing the labels for every cell is wasted work. The mapping is read-only, as zinnia's was, so the weights stay clean, file-backed pages the system can evict; it is sound because the model is a packaged file installed by replacement and never edited in place while the host runs. A failed load is not remembered, so a model installed later is picked up.

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use lru::LruCache;
use memmap2::Mmap;

use super::features::{self, InkStroke};
use super::model::Model;
use super::order_handwriting_candidates;
use crate::{EngineError, Result};

const CANNOT_OPEN: &str = "Cannot open handwriting model";
const INVALID_CANVAS: &str = "Invalid handwriting canvas";
const INVALID_STROKE: &str = "Invalid handwriting stroke";
const INVALID_POINT: &str = "Invalid handwriting point";
const RECOGNITION_FAILED: &str = "Handwriting recognition failed";

const MAX_CANVAS: f32 = 10_000.0;
const MAX_STROKES: usize = 64;
const MAX_STROKE_POINTS: usize = 512;
/// zinnia's canvas, and the box the character's longer side is scaled to inside it, centred at 500.
const INK_CANVAS: u32 = 1000;
const INK_BOX: f32 = 800.0;
const INK_CENTRE: f32 = 500.0;
/// How many classes are scored into the candidate list before `order_handwriting_candidates`.
const NBEST: usize = 12;
const MODEL_CACHE_CAPACITY: usize = 8;

static MODELS: LazyLock<Mutex<LruCache<PathBuf, Arc<Model>>>> = LazyLock::new(|| {
    Mutex::new(LruCache::new(
        NonZeroUsize::new(MODEL_CACHE_CAPACITY).unwrap(),
    ))
});

/// Recognise one character drawn as `strokes` of `(x, y)` points on a `width` by `height` canvas, returning up to 12 candidates ordered by `order_handwriting_candidates`. `model_path` is a trusted packaged zinnia model such as `handwriting-zh_CN.model`.
///
/// As the bridge did: an empty path or no points at all returns no candidates without opening the model, and trailing empty strokes are dropped while an empty stroke between two others is an invalid stroke. Errors carry the reference's messages: a model that cannot be opened is `Failed`, a canvas, stroke or point outside the limits is `InvalidArgument`.
pub fn handwriting_recognize(
    model_path: &str,
    strokes: &[Vec<(f32, f32)>],
    width: f32,
    height: f32,
) -> Result<Vec<String>> {
    let Some(used) = strokes.iter().rposition(|stroke| !stroke.is_empty()) else {
        return Ok(Vec::new());
    };
    if model_path.is_empty() {
        return Ok(Vec::new());
    }
    let model = load(Path::new(model_path))?;
    recognize(&model, &strokes[..=used], width, height)
}

#[allow(unsafe_code)]
fn load(path: &Path) -> Result<Arc<Model>> {
    let mut models = MODELS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(model) = models.get(path) {
        return Ok(Arc::clone(model));
    }
    let file =
        crate::paths::open_file_no_follow(path).map_err(|_| EngineError::failed(CANNOT_OPEN))?;
    // SAFETY: a mapping is only sound while nothing changes the file underneath it. The model is a packaged, read-only file installed by replacement and never written in place (module doc), so the mapped inode keeps its bytes for as long as the map lives.
    let bytes = unsafe { Mmap::map(&file) }.map_err(|_| EngineError::failed(CANNOT_OPEN))?;
    let model = Arc::new(Model::parse(bytes).map_err(|_| EngineError::failed(CANNOT_OPEN))?);
    models.put(path.to_owned(), Arc::clone(&model));
    Ok(model)
}

/// `Recognizer::recognize`: validate, normalise, classify and order.
fn recognize(
    model: &Model,
    strokes: &[Vec<(f32, f32)>],
    width: f32,
    height: f32,
) -> Result<Vec<String>> {
    let Some(ink) = ink(strokes, width, height)? else {
        return Ok(Vec::new());
    };
    if model.is_empty() {
        return Err(EngineError::failed(RECOGNITION_FAILED));
    }
    let features = features::extract(&ink, INK_CANVAS)
        .ok_or_else(|| EngineError::failed(RECOGNITION_FAILED))?;
    let candidates: Vec<String> = model
        .classify(&features, NBEST)
        .into_iter()
        .map(|(label, _)| label.to_owned())
        .collect();
    Ok(order_handwriting_candidates(&candidates))
}

/// Validate the strokes and move the character's bounding box into zinnia's canvas without changing its aspect ratio. `None` for no strokes.
fn ink(strokes: &[Vec<(f32, f32)>], width: f32, height: f32) -> Result<Option<Vec<InkStroke>>> {
    let canvas_ok = |size: f32| size.is_finite() && size > 0.0 && size <= MAX_CANVAS;
    if !canvas_ok(width) || !canvas_ok(height) || strokes.len() > MAX_STROKES {
        return Err(EngineError::invalid(INVALID_CANVAS));
    }
    if strokes.is_empty() {
        return Ok(None);
    }
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (width, height, 0.0_f32, 0.0_f32);
    for stroke in strokes {
        if stroke.is_empty() || stroke.len() > MAX_STROKE_POINTS {
            return Err(EngineError::invalid(INVALID_STROKE));
        }
        for &(x, y) in stroke {
            let inside = x.is_finite()
                && y.is_finite()
                && (0.0..=width).contains(&x)
                && (0.0..=height).contains(&y);
            if !inside {
                return Err(EngineError::invalid(INVALID_POINT));
            }
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    let scale = INK_BOX / (max_x - min_x).max(max_y - min_y).max(1.0);
    let (centre_x, centre_y) = ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0);
    // `Character::add` takes `int`, so the reference truncates toward zero, which `as` does.
    let ink: Vec<InkStroke> = strokes
        .iter()
        .map(|stroke| {
            stroke
                .iter()
                .map(|&(x, y)| {
                    (
                        (INK_CENTRE + (x - centre_x) * scale) as i32,
                        (INK_CENTRE + (y - centre_y) * scale) as i32,
                    )
                })
                .collect()
        })
        .collect();
    Ok(Some(ink))
}

#[cfg(test)]
mod tests {
    use super::super::model::tests::{encode, parse};
    use super::*;

    /// The reference test's pen trajectory for 中 on a 160 by 155 canvas, the central vertical last.
    fn zhong() -> Vec<Vec<(f32, f32)>> {
        vec![
            vec![(35.0, 40.0), (35.0, 105.0)],
            vec![(35.0, 40.0), (125.0, 40.0), (125.0, 105.0)],
            vec![(35.0, 105.0), (125.0, 105.0)],
            vec![(80.0, 15.0), (80.0, 140.0)],
        ]
    }

    fn message(result: Result<Vec<String>>) -> String {
        result.unwrap_err().to_string()
    }

    fn tiny_model() -> Model {
        parse(&encode(&[
            ("甲", 0.0, &[(0, 1.0)]),
            ("x", 0.0, &[(0, 2.0)]),
            ("乙", 0.0, &[(0, 0.5)]),
        ]))
        .unwrap()
    }

    #[test]
    fn validates_like_the_reference() {
        let model = tiny_model();
        assert!(recognize(&model, &[], 160.0, 155.0).unwrap().is_empty());
        // The canvas is checked before the empty early return.
        assert_eq!(message(recognize(&model, &[], 0.0, 155.0)), INVALID_CANVAS);
        for (width, height) in [
            (0.0, 155.0),
            (160.0, -1.0),
            (f32::NAN, 155.0),
            (160.0, f32::INFINITY),
            (10_001.0, 155.0),
        ] {
            assert_eq!(
                message(recognize(&model, &zhong(), width, height)),
                INVALID_CANVAS
            );
        }
        assert!(recognize(&model, &zhong(), 10_000.0, 10_000.0).is_ok());
        let many = vec![vec![(1.0, 1.0)]; MAX_STROKES + 1];
        assert_eq!(
            message(recognize(&model, &many, 160.0, 155.0)),
            INVALID_CANVAS
        );
        assert!(recognize(&model, &many[..MAX_STROKES], 160.0, 155.0).is_ok());
        assert_eq!(
            message(recognize(&model, &[vec![(-1.0, 0.0)]], 160.0, 155.0)),
            INVALID_POINT
        );
        assert_eq!(
            message(recognize(&model, &[vec![(0.0, 155.5)]], 160.0, 155.0)),
            INVALID_POINT
        );
        assert_eq!(
            message(recognize(&model, &[vec![(f32::NAN, 1.0)]], 160.0, 155.0)),
            INVALID_POINT
        );
        assert!(recognize(&model, &[vec![(0.0, 0.0), (160.0, 155.0)]], 160.0, 155.0).is_ok());
        assert_eq!(
            message(recognize(&model, &[vec![(1.0, 1.0)], vec![]], 160.0, 155.0)),
            INVALID_STROKE
        );
        let long = vec![vec![(1.0, 1.0); MAX_STROKE_POINTS + 1]];
        assert_eq!(
            message(recognize(&model, &long, 160.0, 155.0)),
            INVALID_STROKE
        );
        // Strokes are checked in order, so an earlier bad point wins over a later empty stroke.
        assert_eq!(
            message(recognize(
                &model,
                &[vec![(-1.0, 0.0)], vec![]],
                160.0,
                155.0
            )),
            INVALID_POINT
        );
    }

    #[test]
    fn classifies_and_orders_candidates() {
        // Only the bias feature matches, so the scores are the weights: x 2.0, 甲 1.0, 乙 0.5; the CJK labels are then moved first.
        let ordered = recognize(&tiny_model(), &zhong(), 160.0, 155.0).unwrap();
        assert_eq!(ordered, vec!["甲", "乙", "x"]);
        let empty = parse(&encode(&[])).unwrap();
        assert_eq!(
            message(recognize(&empty, &zhong(), 160.0, 155.0)),
            RECOGNITION_FAILED
        );
    }

    #[test]
    fn bridge_entry_skips_empty_input_and_trims_trailing_strokes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tiny.model");
        std::fs::write(&path, encode(&[("甲", 0.0, &[(0, 1.0)])])).unwrap();
        let path = path.to_str().unwrap();

        assert!(handwriting_recognize(path, &[], 160.0, 155.0)
            .unwrap()
            .is_empty());
        assert!(handwriting_recognize(path, &[vec![], vec![]], 0.0, 155.0)
            .unwrap()
            .is_empty());
        assert!(handwriting_recognize("", &zhong(), 160.0, 155.0)
            .unwrap()
            .is_empty());
        let mut trailing = zhong();
        trailing.push(Vec::new());
        assert_eq!(
            handwriting_recognize(path, &trailing, 160.0, 155.0).unwrap(),
            vec!["甲"]
        );
        let mut inner = zhong();
        inner.insert(1, Vec::new());
        assert_eq!(
            message(handwriting_recognize(path, &inner, 160.0, 155.0)),
            INVALID_STROKE
        );
    }

    fn parse_strokes(body: &str) -> Vec<Vec<(f32, f32)>> {
        body.split(';')
            .map(|stroke| {
                stroke
                    .split(' ')
                    .map(|point| {
                        let (x, y) = point.split_once(',').unwrap();
                        (x.parse().unwrap(), y.parse().unwrap())
                    })
                    .collect()
            })
            .collect()
    }

    /// Parity with the C++ zinnia on the shipped model. `testdata/strokes.tsv` holds the reference test's 中 (and its moved copy) plus deterministic, densely sampled, slightly wobbling trajectories that exercise the vertex splitting; `testdata/zinnia-12best.tsv` is the reference's raw 12-best with scores for each line, from `handwriting.cpp`'s normalisation over the vendored zinnia sources built with clang `-O2 -ffp-contract=off`. Needs the model in `MSIME_HANDWRITING_MODEL` and is skipped, with the reason printed, without it.
    #[test]
    fn matches_zinnia_on_the_shipped_model() {
        let Some(path) = std::env::var_os("MSIME_HANDWRITING_MODEL") else {
            eprintln!("skipped: set MSIME_HANDWRITING_MODEL to handwriting-zh_CN.model");
            return;
        };
        let model = load(Path::new(&path)).unwrap();
        let fixtures = include_str!("testdata/strokes.tsv").lines();
        let expected = include_str!("testdata/zinnia-12best.tsv").lines();
        let mut compared = 0;
        for (fixture, expected) in fixtures.zip(expected) {
            let fields: Vec<&str> = fixture.split('\t').collect();
            let strokes = parse_strokes(fields[3]);
            let ink = ink(
                &strokes,
                fields[1].parse().unwrap(),
                fields[2].parse().unwrap(),
            )
            .unwrap()
            .unwrap();
            let features = features::extract(&ink, INK_CANVAS).unwrap();
            let actual = model.classify(&features, NBEST);
            let mut columns = expected.split('\t');
            assert_eq!(columns.next(), Some(fields[0]));
            let expected: Vec<(&str, f32)> = columns
                .map(|column| {
                    let (label, score) = column.split_once(' ').unwrap();
                    (label, score.parse().unwrap())
                })
                .collect();
            let labels = |items: &[(&str, f32)]| {
                items
                    .iter()
                    .map(|item| item.0.to_owned())
                    .collect::<Vec<_>>()
            };
            assert_eq!(labels(&actual), labels(&expected), "{fixture}");
            for ((_, actual), (_, expected)) in actual.iter().zip(&expected) {
                // Exact on the platform the reference ran on; the slack is for another libm's `atan2f`.
                assert!(
                    (actual - expected).abs() <= 1e-5,
                    "{fixture}: {actual} vs {expected}"
                );
            }
            compared += 1;
        }
        assert_eq!(compared, 22);

        // The reference test itself: 中 is found on its canvas and when moved and shrunk on a wider one.
        let written =
            handwriting_recognize(path.to_str().unwrap(), &zhong(), 160.0, 155.0).unwrap();
        assert!(
            written.iter().any(|candidate| candidate == "中"),
            "{written:?}"
        );
        let moved: Vec<Vec<(f32, f32)>> = zhong()
            .into_iter()
            .map(|stroke| {
                stroke
                    .into_iter()
                    .map(|(x, y)| (x * 0.7 + 180.0, y * 0.7 + 10.0))
                    .collect()
            })
            .collect();
        let moved = handwriting_recognize(path.to_str().unwrap(), &moved, 400.0, 155.0).unwrap();
        assert!(moved.iter().any(|candidate| candidate == "中"), "{moved:?}");
    }

    /// A model is mapped once per path: a second load returns the same `Arc`, and the cached model still answers after its file is unlinked, since the mapping keeps the inode.
    #[test]
    fn a_model_is_mapped_once_per_path() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("once.model");
        std::fs::write(&path, encode(&[("甲", 0.0, &[(0, 1.0)])])).unwrap();
        let first = load(&path).unwrap();
        let second = load(&path).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            handwriting_recognize(path.to_str().unwrap(), &zhong(), 160.0, 155.0).unwrap(),
            vec!["甲"]
        );
        // A directory is not a model.
        assert_eq!(
            message(handwriting_recognize(
                directory.path().to_str().unwrap(),
                &zhong(),
                160.0,
                155.0
            )),
            CANNOT_OPEN
        );
    }

    #[test]
    fn the_model_cache_is_bounded_across_paths() {
        let mut directories = Vec::new();
        for index in 0..=MODEL_CACHE_CAPACITY {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join(format!("model-{index}.model"));
            std::fs::write(&path, encode(&[("甲", 0.0, &[(0, 1.0)])])).unwrap();
            assert!(load(&path).is_ok());
            directories.push(directory);
        }
        let cache = MODELS.lock().unwrap();
        assert!(cache.len() <= MODEL_CACHE_CAPACITY);
    }

    #[test]
    fn unreadable_models_fail_to_open() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("missing-handwriting-model");
        let error =
            handwriting_recognize(missing.to_str().unwrap(), &zhong(), 160.0, 155.0).unwrap_err();
        assert!(matches!(error, EngineError::Failed(ref text) if text == CANNOT_OPEN));
        let broken = directory.path().join("broken.model");
        std::fs::write(&broken, b"not a zinnia model").unwrap();
        assert_eq!(
            message(handwriting_recognize(
                broken.to_str().unwrap(),
                &zhong(),
                160.0,
                155.0
            )),
            CANNOT_OPEN
        );
        // The model is opened before the canvas is checked, as the reference constructed its recognizer first.
        assert_eq!(
            message(handwriting_recognize(
                broken.to_str().unwrap(),
                &zhong(),
                0.0,
                155.0
            )),
            CANNOT_OPEN
        );
        // A failure is not cached: once a model appears at the path it loads.
        std::fs::write(&missing, encode(&[("甲", 0.0, &[])])).unwrap();
        assert_eq!(
            handwriting_recognize(missing.to_str().unwrap(), &zhong(), 160.0, 155.0).unwrap(),
            vec!["甲"]
        );
    }
}
