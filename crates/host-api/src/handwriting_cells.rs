//! Splits a line of handwriting into character cells so the single-character Engine recognizer can offer multi-character candidates, the way the Windows Ink recognizer segments a written line.
//!
//! Segmentation only runs when the ink is clearly a line: at least 1.6 times as long along one axis as across it. Square or near-square ink stays one cell and goes down the single-character path unchanged.

/// The candidate cap of the shared handwriting panel contract (`HandwritingRecognitionResult::validate`).
pub(crate) const MAX_HANDWRITING_CANDIDATES: usize = 12;

/// Ink this much longer along one axis than across it is treated as a line of characters.
const LINE_ASPECT: f32 = 1.6;
/// Lower bound for the estimated character size, so a line of flat strokes does not shrink the character size to nothing.
const MIN_CHARACTER_SIZE: f32 = 24.0;
/// Boxes closer than this fraction of the character size belong to the same component (女 and 子 in 好).
const JOIN_GAP: f32 = 0.12;
/// Neighbouring components are one character while their combined extent stays within this fraction of the character size (radicals written apart).
const MERGE_EXTENT: f32 = 1.15;
/// A component longer than this fraction of the character size holds touching characters and is split at its widest gap.
const SPLIT_EXTENT: f32 = 1.8;
/// More cells than this is not a line the recognizer can handle cell by cell; the ink is then classified whole.
const MAX_CELLS: usize = 8;

#[derive(Clone, Copy)]
struct Span {
    stroke: usize,
    start: f32,
    end: f32,
}

/// Group stroke indices into character cells along the writing direction. Each cell keeps the original stroke order. Ink that is not clearly a line, or that splits into more than `MAX_CELLS` cells, comes back as a single cell holding every stroke.
pub(crate) fn segment_handwriting_cells(strokes: &[Vec<(f32, f32)>]) -> Vec<Vec<usize>> {
    let whole = || vec![(0..strokes.len()).collect::<Vec<_>>()];
    let mut boxes = Vec::with_capacity(strokes.len());
    boxes.extend(strokes.iter().enumerate().filter_map(|(index, stroke)| {
        let first = stroke.first()?;
        let init = (first.0, first.1, first.0, first.1);
        let (min_x, min_y, max_x, max_y) = stroke.iter().fold(init, |(a, b, c, d), &(x, y)| {
            (a.min(x), b.min(y), c.max(x), d.max(y))
        });
        Some((index, min_x, min_y, max_x, max_y))
    }));
    if boxes.len() < 2 {
        return whole();
    }
    let min_x = boxes.iter().map(|b| b.1).fold(f32::INFINITY, f32::min);
    let min_y = boxes.iter().map(|b| b.2).fold(f32::INFINITY, f32::min);
    let max_x = boxes.iter().map(|b| b.3).fold(f32::NEG_INFINITY, f32::max);
    let max_y = boxes.iter().map(|b| b.4).fold(f32::NEG_INFINITY, f32::max);
    let (width, height) = (max_x - min_x, max_y - min_y);
    if !width.is_finite() || !height.is_finite() {
        return whole();
    }
    let horizontal = if width >= LINE_ASPECT * height {
        true
    } else if height >= LINE_ASPECT * width {
        false
    } else {
        return whole();
    };
    let size = if horizontal { height } else { width }.max(MIN_CHARACTER_SIZE);
    let mut spans = Vec::with_capacity(boxes.len());
    spans.extend(boxes.iter().map(|&(stroke, x0, y0, x1, y1)| {
        let (start, end) = if horizontal { (x0, x1) } else { (y0, y1) };
        Span { stroke, start, end }
    }));
    spans.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.stroke.cmp(&b.stroke)));

    // Pass 1: strokes that overlap or nearly touch along the writing axis form one component.
    let mut components: Vec<Vec<Span>> = Vec::with_capacity(spans.len());
    for span in spans {
        match components.last_mut() {
            Some(component) if span.start <= extent(component).1 + JOIN_GAP * size => {
                component.push(span)
            }
            _ => components.push(vec![span]),
        }
    }

    // Pass 2: neighbouring components that still fit in one character are one character.
    let mut cells: Vec<Vec<Span>> = Vec::with_capacity(components.len());
    for component in components {
        match cells.last_mut() {
            Some(cell) => {
                let (start, end) = extent(cell);
                if end.max(extent(&component).1) - start <= MERGE_EXTENT * size {
                    cell.extend(component);
                } else {
                    cells.push(component);
                }
            }
            None => cells.push(component),
        }
    }

    // Pass 3: a cell too long for one character holds touching characters.
    let mut split = Vec::with_capacity(cells.len());
    for cell in cells {
        split_long_cell(cell, size, &mut split);
    }
    if split.len() < 2 || split.len() > MAX_CELLS {
        return whole();
    }
    let mut result = Vec::with_capacity(split.len());
    result.extend(split.into_iter().map(|cell| {
        let mut strokes = Vec::with_capacity(cell.len());
        strokes.extend(cell.into_iter().map(|span| span.stroke));
        strokes.sort_unstable();
        strokes
    }));
    result
}

fn extent(spans: &[Span]) -> (f32, f32) {
    spans
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(start, end), span| {
            (start.min(span.start), end.max(span.end))
        })
}

/// Split a start-sorted cell at its widest gap until every piece fits in `SPLIT_EXTENT` characters or is a single stroke.
fn split_long_cell(cell: Vec<Span>, size: f32, out: &mut Vec<Vec<Span>>) {
    let (start, end) = extent(&cell);
    if cell.len() < 2 || end - start <= SPLIT_EXTENT * size {
        out.push(cell);
        return;
    }
    let mut reach = cell[0].end;
    let mut best = (1, f32::NEG_INFINITY);
    for (index, span) in cell.iter().enumerate().skip(1) {
        let gap = span.start - reach;
        if gap > best.1 {
            best = (index, gap);
        }
        reach = reach.max(span.end);
    }
    let mut left = cell;
    let right = left.split_off(best.0);
    split_long_cell(left, size, out);
    split_long_cell(right, size, out);
}

/// Combine each cell's ordered candidates into line candidates: every cell's top choice first, then the variants that swap one cell to its next choice, rank by rank and round-robin across the cells in writing order. The Engine reports only ordered strings, not scores, so rank is the confidence measure. The result is deduplicated and capped at `MAX_HANDWRITING_CANDIDATES`; a single cell passes through as is. Any cell without candidates makes the whole result empty, so the caller can fall back to classifying the ink whole.
pub(crate) fn combine_cell_candidates(cells: Vec<Vec<String>>) -> Vec<String> {
    if cells.len() == 1 {
        let mut single = cells.into_iter().next().unwrap_or_default();
        single.truncate(MAX_HANDWRITING_CANDIDATES);
        return single;
    }
    if cells.is_empty() || cells.iter().any(Vec::is_empty) {
        return Vec::new();
    }
    let mut best = Vec::with_capacity(cells.len());
    best.extend(cells.iter().map(|cell| cell[0].as_str()));
    let mut result = Vec::with_capacity(MAX_HANDWRITING_CANDIDATES);
    result.push(best.concat());
    let deepest = cells.iter().map(Vec::len).max().unwrap_or(0);
    'ranks: for rank in 1..deepest {
        for (index, cell) in cells.iter().enumerate() {
            let Some(choice) = cell.get(rank) else {
                continue;
            };
            let mut variant = best.clone();
            variant[index] = choice;
            let variant = variant.concat();
            if !result.contains(&variant) {
                result.push(variant);
                if result.len() == MAX_HANDWRITING_CANDIDATES {
                    break 'ranks;
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A box-shaped stroke outline, enough for segmentation which only reads bounding boxes.
    fn stroke(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<(f32, f32)> {
        vec![(x0, y0), (x1, y1)]
    }

    /// Three strokes filling a square glyph at `(x, y)` with side `side`.
    fn glyph(x: f32, y: f32, side: f32) -> Vec<Vec<(f32, f32)>> {
        vec![
            stroke(x, y + side * 0.2, x + side, y + side * 0.2),
            stroke(x + side * 0.5, y, x + side * 0.5, y + side),
            stroke(x, y + side * 0.8, x + side, y + side * 0.8),
        ]
    }

    #[test]
    fn a_square_glyph_is_one_cell() {
        let strokes = glyph(40.0, 40.0, 150.0);
        assert_eq!(segment_handwriting_cells(&strokes), vec![vec![0, 1, 2]]);
    }

    #[test]
    fn a_single_long_stroke_is_one_cell() {
        let strokes = vec![stroke(20.0, 200.0, 400.0, 210.0)];
        assert_eq!(segment_handwriting_cells(&strokes), vec![vec![0]]);
    }

    #[test]
    fn two_glyphs_side_by_side_are_two_cells_in_writing_order() {
        let mut strokes = glyph(230.0, 130.0, 150.0);
        strokes.extend(glyph(20.0, 130.0, 150.0));
        assert_eq!(
            segment_handwriting_cells(&strokes),
            vec![vec![3, 4, 5], vec![0, 1, 2]]
        );
    }

    #[test]
    fn a_left_right_compound_with_a_small_gap_is_one_cell() {
        // Two narrow parts with a 15 px gap between them, just above the join threshold, and about one character wide in total, so pass 2 must join them.
        let strokes = vec![
            stroke(40.0, 90.0, 85.0, 90.0),
            stroke(62.0, 40.0, 62.0, 150.0),
            stroke(100.0, 40.0, 150.0, 40.0),
            stroke(125.0, 40.0, 125.0, 150.0),
            // A trailing glyph far to the right makes the whole ink a horizontal line.
            stroke(230.0, 40.0, 340.0, 40.0),
            stroke(285.0, 40.0, 285.0, 150.0),
        ];
        assert_eq!(
            segment_handwriting_cells(&strokes),
            vec![vec![0, 1, 2, 3], vec![4, 5]]
        );
    }

    #[test]
    fn three_glyphs_stacked_vertically_are_three_cells() {
        let mut strokes = glyph(150.0, 10.0, 110.0);
        strokes.extend(glyph(150.0, 150.0, 110.0));
        strokes.extend(glyph(150.0, 290.0, 110.0));
        assert_eq!(
            segment_handwriting_cells(&strokes),
            vec![vec![0, 1, 2], vec![3, 4, 5], vec![6, 7, 8]]
        );
    }

    #[test]
    fn a_late_stroke_of_the_first_glyph_lands_in_the_first_cell() {
        let mut first = glyph(20.0, 130.0, 150.0);
        let late = first.pop().unwrap();
        let mut strokes = first;
        strokes.extend(glyph(230.0, 130.0, 150.0));
        strokes.push(late);
        assert_eq!(
            segment_handwriting_cells(&strokes),
            vec![vec![0, 1, 5], vec![2, 3, 4]]
        );
    }

    #[test]
    fn touching_glyphs_are_split_at_the_widest_gap() {
        // Three glyphs whose gaps are below the join threshold, so only the long-cell split can separate them.
        let mut strokes = glyph(10.0, 150.0, 120.0);
        strokes.extend(glyph(135.0, 150.0, 120.0));
        strokes.extend(glyph(260.0, 150.0, 120.0));
        assert_eq!(
            segment_handwriting_cells(&strokes),
            vec![vec![0, 1, 2], vec![3, 4, 5], vec![6, 7, 8]]
        );
    }

    #[test]
    fn more_cells_than_the_cap_fall_back_to_one_cell() {
        let strokes = (0..9)
            .map(|index| {
                stroke(
                    index as f32 * 45.0,
                    200.0,
                    index as f32 * 45.0 + 20.0,
                    225.0,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            segment_handwriting_cells(&strokes),
            vec![(0..9).collect::<Vec<_>>()]
        );
    }

    fn owned(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn the_best_combination_comes_first_then_single_swaps_round_robin() {
        let combined =
            combine_cell_candidates(vec![owned(&["你", "尔", "称"]), owned(&["好", "妈", "奴"])]);
        assert_eq!(combined, owned(&["你好", "尔好", "你妈", "称好", "你奴"]));
    }

    #[test]
    fn combinations_are_deduplicated_and_capped() {
        let cell = owned(&[
            "一", "二", "三", "四", "五", "六", "七", "八", "九", "十", "百", "千",
        ]);
        let combined = combine_cell_candidates(vec![cell.clone(), cell.clone(), cell]);
        assert_eq!(combined.len(), MAX_HANDWRITING_CANDIDATES);
        assert_eq!(combined[0], "一一一");
        let mut unique = combined.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), combined.len());

        let repeated = combine_cell_candidates(vec![owned(&["口", "口"]), owned(&["日"])]);
        assert_eq!(repeated, owned(&["口日"]));
    }

    #[test]
    fn a_single_cell_passes_through_and_an_empty_cell_yields_nothing() {
        let single = owned(&["中", "申", "甲"]);
        assert_eq!(combine_cell_candidates(vec![single.clone()]), single);
        assert!(combine_cell_candidates(vec![owned(&["中"]), Vec::new()]).is_empty());
    }
}
