//! 中英混合滑行的英文词表几何匹配，复用拼音滑行的轨迹度量。

use super::*;

/// 英文词表与拼音共用同一套轨迹几何；英文的连续重复字母不要求手指重复画一遍。
pub(crate) struct GlideMatcher(Decoder);

impl GlideMatcher {
    pub(crate) fn new(keyboard: &GlideKeyboard, points: &[GlidePoint]) -> Option<Self> {
        if !keyboard.is_valid()
            || points.len() < 2
            || points
                .iter()
                .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return None;
        }
        let points: Vec<_> = points
            .iter()
            .map(|point| GlidePoint {
                x: point.x / keyboard.key_width,
                y: point.y / keyboard.key_height,
                time_ms: point.time_ms,
            })
            .collect();
        let samples = resample(&points);
        (samples.len() >= 2).then(|| Self(Decoder::new(keyboard.normalized_centers(), &samples)))
    }

    /// 只查询起点附近的首字母，每个首字母的词数另有上限。
    pub(crate) fn starting_letters(&self) -> Vec<u8> {
        (0..26)
            .filter(|&letter| self.0.reach[letter].first() == Some(&0))
            .map(|letter| b'a' + letter as u8)
            .collect()
    }

    pub(crate) fn score(&self, word: &str) -> Option<f32> {
        if !(2..=MAX_GLIDE_LETTERS).contains(&word.len())
            || !word.bytes().all(|letter| letter.is_ascii_lowercase())
        {
            return None;
        }
        let mut letters: Vec<_> = word
            .bytes()
            .map(|letter| usize::from(letter - b'a'))
            .collect();
        letters.dedup();
        let (&first, rest) = letters.split_first()?;
        let count = self.0.count;
        let first_align = self.0.align[first].as_ref()?;
        let last = *letters.last()?;
        if self.0.reach[first].first() != Some(&0)
            || self.0.reach[last].last() != Some(&(count - 1))
        {
            return None;
        }
        let mut costs = [f32::INFINITY; MAX_SAMPLES];
        costs[0] = first_align[0] + LETTER_COST;
        let mut from = first;
        for &to in rest {
            let line = self.0.lines[from * 26 + to].as_ref()?;
            let align = self.0.align[to].as_ref()?;
            let mut next = [f32::INFINITY; MAX_SAMPLES];
            let mut best = f32::INFINITY;
            for at in 1..count {
                best = best.min(costs[at - 1] - line[at - 1]);
                if self.0.reach[to].binary_search(&at).is_ok() {
                    next[at] = best + line[at - 1] + align[at] + LETTER_COST;
                }
            }
            costs = next;
            from = to;
        }
        let score = costs[count - 1];
        score.is_finite().then_some(score)
    }
}
