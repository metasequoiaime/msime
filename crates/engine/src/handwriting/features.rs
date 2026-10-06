//! zinnia's feature extraction (`feature.cpp` `Features::read`), ported for identical scores.
//!
//! Ported from zinnia (https://github.com/taku910/zinnia at 581faa8f), Copyright (c) 2005-2007 Taku Kudo, under its 3-clause BSD license, whose text ships with the product notices as `Zinnia-LICENSE.txt`.
//!
//! The mixed precision is deliberate and is what the model was trained against: the reference stores coordinates as `float` but computes every expression involving the `double` literal `0.5` in double before truncating the result to `float`. Each function below keeps the reference's precision per expression. Rust never fuses a multiply and an add, so the results match a reference built without floating-point contraction (the x86-64 default).

/// One sparse feature, `FeatureNode` in the reference. Model weights use the same shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct FeatureNode {
    pub index: i32,
    pub value: f32,
}

/// One stroke of integer points on zinnia's canvas, `Character::add` in the reference.
pub(super) type InkStroke = Vec<(i32, i32)>;

#[derive(Clone, Copy)]
struct Node {
    x: f32,
    y: f32,
}

/// `kMaxCharacterSize`: vertex pairs with an id above this are never turned into features.
const MAX_VERTEX_ID: usize = 50;
/// The squared, canvas-normalised distance above which a segment is split again.
const SPLIT_ERROR: f32 = 0.001;

/// The features of one character whose strokes are integer points on a `size` by `size` canvas, sorted by index and without the reference's `-1` terminator. `None` where the reference's `read` returns false: no strokes or an empty stroke.
///
/// Indices can repeat in one case, a vertex pair with id 50, whose offset `sid * 1000 + 1000` equals the next stroke's first pair. The sort here is stable, so the earlier feature is the one `dot` uses; the reference's `std::sort` leaves that order to the standard library.
pub(super) fn extract(strokes: &[InkStroke], size: u32) -> Option<Vec<FeatureNode>> {
    if strokes.is_empty() || strokes.iter().any(Vec::is_empty) {
        return None;
    }
    let size = f64::from(size);
    let nodes: Vec<Vec<Node>> = strokes
        .iter()
        .map(|stroke| {
            stroke
                .iter()
                .map(|&(x, y)| Node {
                    x: (f64::from(x) / size) as f32,
                    y: (f64::from(y) / size) as f32,
                })
                .collect()
        })
        .collect();

    let mut features = vec![FeatureNode {
        index: 0,
        value: 1.0,
    }];
    let mut previous_last: Option<Node> = None;
    for (sid, stroke) in nodes.iter().enumerate() {
        let sid = sid as i32;
        let mut pairs = [None; MAX_VERTEX_ID + 1];
        vertices(stroke, 0, stroke.len() - 1, 0, &mut pairs);
        let pair_count = pairs.iter().filter(|pair| pair.is_some()).count();
        let move_count = usize::from(previous_last.is_some());
        features.reserve_exact(pair_count.saturating_add(move_count).saturating_mul(12));
        for (id, pair) in pairs.iter().enumerate() {
            if let Some((first, last)) = pair {
                basic_features(
                    &mut features,
                    sid * 1000 + 20 * id as i32,
                    stroke[*first],
                    stroke[*last],
                );
            }
        }
        if let Some(previous) = previous_last {
            basic_features(&mut features, 100_000 + sid * 1000, previous, stroke[0]);
        }
        previous_last = Some(stroke[stroke.len() - 1]);
    }
    let stroke_count = nodes.len() as i32;
    features.reserve_exact(2);
    push(&mut features, 2_000_000, stroke_count as f32);
    push(&mut features, 2_000_000 + stroke_count, 10.0);
    features.sort_by_key(|feature| feature.index);
    Some(features)
}

fn push(features: &mut Vec<FeatureNode>, index: i32, value: f32) {
    features.push(FeatureNode { index, value });
}

/// `getVertex`: record the segment `first..=last` under `id`, then split it at its farthest point while that point is far enough from the chord. Children of `id` have larger ids, so a subtree above `MAX_VERTEX_ID` is skipped: the reference computes it and then discards it, and skipping it also bounds the recursion on pathological strokes.
fn vertices(
    stroke: &[Node],
    first: usize,
    last: usize,
    id: usize,
    pairs: &mut [Option<(usize, usize)>; MAX_VERTEX_ID + 1],
) {
    if id > MAX_VERTEX_ID {
        return;
    }
    pairs[id] = Some((first, last));
    let (distance, best) = minimum_distance(stroke, first, last);
    // A NaN distance (a closed stroke whose points all sit on the start) does not split, as in the reference.
    if distance > SPLIT_ERROR {
        vertices(stroke, first, best, id * 2 + 1, pairs);
        vertices(stroke, best, last, id * 2 + 2, pairs);
    }
}

/// `minimum_distance`: the squared distance, relative to the chord length, of the point in `first..last` (the last point excluded) farthest from the chord, and that point. Ties keep the earliest point.
fn minimum_distance(stroke: &[Node], first: usize, last: usize) -> (f32, usize) {
    if first == last {
        return (0.0, first);
    }
    let (f, l) = (stroke[first], stroke[last]);
    let a = l.x - f.x;
    let b = l.y - f.y;
    let c = (f64::from(l.y * f.x) - f64::from(l.x * f.y)) as f32;
    let mut max = -1.0_f32;
    let mut best = first;
    for (index, node) in stroke.iter().enumerate().take(last).skip(first) {
        let distance = ((a * node.y) - (b * node.x) + c).abs();
        if distance > max {
            max = distance;
            best = index;
        }
    }
    (max * max / (a * a + b * b), best)
}

/// `makeBasicFeature`: twelve features describing the segment from `first` to `last`, at `offset + 1..=12`.
fn basic_features(features: &mut Vec<FeatureNode>, offset: i32, first: Node, last: Node) {
    let centred = |value: f32| f64::from(value) - 0.5;
    push(features, offset + 1, 10.0 * distance(first, last));
    push(
        features,
        offset + 2,
        (last.y - first.y).atan2(last.x - first.x),
    );
    push(features, offset + 3, (10.0 * centred(first.x)) as f32);
    push(features, offset + 4, (10.0 * centred(first.y)) as f32);
    push(features, offset + 5, (10.0 * centred(last.x)) as f32);
    push(features, offset + 6, (10.0 * centred(last.y)) as f32);
    push(
        features,
        offset + 7,
        centred(first.y).atan2(centred(first.x)) as f32,
    );
    push(
        features,
        offset + 8,
        centred(last.y).atan2(centred(last.x)) as f32,
    );
    push(features, offset + 9, 10.0 * distance_to_centre(first));
    push(features, offset + 10, 10.0 * distance_to_centre(last));
    push(features, offset + 11, 5.0 * (last.x - first.x));
    push(features, offset + 12, 5.0 * (last.y - first.y));
}

fn distance(first: Node, last: Node) -> f32 {
    let x = first.x - last.x;
    let y = first.y - last.y;
    (x * x + y * y).sqrt()
}

/// `distance2`: the offsets from the centre are computed in double and stored as float, the rest is float.
fn distance_to_centre(node: Node) -> f32 {
    let x = (f64::from(node.x) - 0.5) as f32;
    let y = (f64::from(node.y) - 0.5) as f32;
    (x * x + y * y).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn indices(features: &[FeatureNode]) -> Vec<i32> {
        features.iter().map(|feature| feature.index).collect()
    }

    #[test]
    fn rejects_what_the_reference_rejects() {
        assert!(extract(&[], 1000).is_none());
        assert!(extract(&[vec![(1, 1)], vec![]], 1000).is_none());
    }

    #[test]
    fn straight_stroke_has_one_vertex_pair() {
        let features = extract(&[vec![(100, 500), (500, 500), (900, 500)]], 1000).unwrap();
        let mut expected = vec![0];
        expected.extend(1..=12);
        expected.extend([2_000_000, 2_000_001]);
        assert_eq!(indices(&features), expected);
        assert_eq!(features[0].value, 1.0);
        // Length 0.8 of the canvas, horizontal, from (0.1, 0.5) to (0.9, 0.5).
        assert!((features[1].value - 8.0).abs() < 1e-5);
        assert_eq!(features[2].value, 0.0);
        assert!((features[3].value + 4.0).abs() < 1e-5);
        assert!((features[11].value - 4.0).abs() < 1e-5);
        assert_eq!(features[13].value, 1.0);
        assert_eq!(features[14].value, 10.0);
    }

    #[test]
    fn corner_splits_into_children_and_second_stroke_adds_move() {
        let corner = vec![(100, 100), (900, 100), (900, 900)];
        let features = extract(&[corner, vec![(200, 800), (800, 800)]], 1000).unwrap();
        let blocks: Vec<i32> = indices(&features)
            .into_iter()
            .filter(|index| index % 20 == 1 || *index > 1_999_999)
            .collect();
        // Stroke 0: pair 0 and its two children 1 and 2; stroke 1: pair 0 at 1000, the move at 101000; then the stroke count.
        assert_eq!(blocks, vec![1, 21, 41, 1001, 101_001, 2_000_000, 2_000_002]);
    }

    #[test]
    fn closed_stroke_does_not_split() {
        let features = extract(&[vec![(500, 500), (900, 900), (500, 500)]], 1000).unwrap();
        assert_eq!(features.len(), 1 + 12 + 2);
    }

    #[test]
    fn deep_zigzag_stops_at_the_vertex_limit() {
        let zigzag: Vec<(i32, i32)> = (0..512)
            .map(|index| (index * 1000 / 512, if index % 2 == 0 { 100 } else { 900 }))
            .collect();
        let features = extract(&[zigzag], 1000).unwrap();
        assert!(features
            .iter()
            .all(|feature| feature.index <= 1012 || feature.index >= 2_000_000));
        let pairs = features
            .iter()
            .filter(|feature| feature.index < 2_000_000 && feature.index % 20 == 1)
            .count();
        assert!(pairs > 5, "{pairs}");
    }

    #[test]
    fn feature_output_reserves_generated_nodes() {
        let zigzag: Vec<(i32, i32)> = (0..512)
            .map(|index| (index * 1000 / 512, if index % 2 == 0 { 100 } else { 900 }))
            .collect();
        let features = extract(&[zigzag], 1000).unwrap();
        assert_eq!(features.capacity(), features.len());
    }
}
