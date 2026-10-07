//! 滑行输入：手指一笔滑过若干字母键，解码成它最可能拼出的全拼字母串，代价低的在前。
//!
//! 解码器只看几何，只认得音节表；按词典给这些字母串排序是会话的事。一个假设是一串字母，加上每个字母对齐到的轨迹采样点：第一个字母在第一个采样点，最后一个在最后一个，其余按次序落在中间。它的代价有四部分：
//!
//! - 每个对齐点付它到字母键中心距离的平方，所以笔画可以偏离键位一部分键宽；
//! - 两个对齐字母之间的每个采样点付它到两键连线距离的平方，所以笔画拐弯的地方必须对齐一个字母，而笔画没有拐弯的键只有恰好在连线上时才不额外付费；沿两键方向往回走也要付费，所以沿一条线来回折返的笔画（`keneng`）不会被读成只走了一遍（`keng`）；
//! - 对齐时跳过的每一次停留（手指在键上停住）付固定罚分，所以在键上停一下，即使在直线上也能确认这个键；
//! - 每个字母付一个小的固定价，所以笔画走得同样好的两个串里短的那个胜出，只是经过的键不会被当成按下。
//!
//! 坐标先除以键宽和键高，不论宿主的像素、密度和键距如何，一个键都是单位正方形。搜索对每个采样点只保留对齐在那里的少数几个最便宜的前缀（令牌传递），连线代价来自前缀和，所以一次扩展的耗时只与这个字母键附近的采样点数有关。

use std::collections::HashMap;
use std::sync::OnceLock;

use super::syllables::{intact_pinyin_list, is_intact, MAX_SYLLABLE_LENGTH};

/// 一笔最多重采样成多少个点；更长的笔画改为更稀的采样。
const MAX_SAMPLES: usize = 128;
/// 重采样的步长，以键为单位。
const SAMPLE_STEP: f32 = 0.25;
/// 对齐一个字母时代价为 1 所对应的距离平方（键为单位），约半个键。
const ALIGN_SCALE: f32 = 0.3;
/// 离两键连线多远（距离平方，键为单位）时，每单位笔画长度的代价为 1。
const LINE_SCALE: f32 = 0.25;
/// 每个字母的固定价。取得小：落在另外两个键连线上的键（`z` 与 `o` 之间的 `h`）对齐它只比跳过它贵一点，`zhong` 与 `zong` 应当由词典而不是几何来分。
const LETTER_COST: f32 = 0.8;
/// 字母与前一个相同时的额外价。笔画画不出连按两次，没有它的话每个经过的键都会被再读一遍（`de` 读成 `dee`）。拼音只在音节边界上出现重复字母，但那里并不少见（`zhong'guo`、`nan'ning`），所以它只需让重复的读法排在不重复的后面，其余交给词典。
const REPEAT_COST: f32 = 0.8;
/// [`rank_by_dictionary`] 重排时，词典权重的一个对数单位抵多少几何代价。
const DICTIONARY_WEIGHT: f32 = 0.7;
/// 不是词典词的串在按最罕见音节计分之外再付的代价。
const UNKNOWN_WORD_COST: f32 = 6.0;
/// 词典词每多一个音节，对数权重加多少。出货词典里常用词的权重在五十万上下、常用字在几百万，没有这一项的话，只要 `y` 落在笔画的连线上，企业（`qi'ye`）就会输给切（`qie`）。
const WORD_SYLLABLE_BONUS: f32 = 1.5;
/// 笔画逆着一个对齐键到下一个对齐键的方向每走一个键所付的代价。只有连线代价时，笔画沿一条线来回走不花钱，于是在键盘一行上来回两次的 `laishuo` 被读成了 `lao`。
const BACKTRACK_COST: f32 = 2.0;
/// 没有字母对齐到的停留付多少。
const SKIPPED_DWELL_COST: f32 = 4.0;
/// 离每个采样点都比这远（键为单位）的键不可能在串里。
const NEAR_DISTANCE: f32 = 1.4;
/// 每个采样点保留的前缀数。
const TOKENS_PER_SAMPLE: usize = 6;
/// 比同一采样点上最便宜的前缀贵出这么多的前缀会被丢掉。
const BEAM: f32 = 10.0;
/// 考虑的最长字母串；拼出更长的笔画不解码。
pub const MAX_GLIDE_LETTERS: usize = 30;
/// 手指在离停下处 [`DWELL_RADIUS`] 以内停这么久算一次停留。
const DWELL_MS: u32 = 120;
const DWELL_RADIUS: f32 = 0.35;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlidePoint {
    pub x: f32,
    pub y: f32,
    /// 距笔画开始的毫秒数，宿主量了才有；只有带时间才能找出停留。
    pub time_ms: Option<u32>,
}

/// 宿主画出来的字母键，用宿主自己的坐标。
#[derive(Debug, Clone, PartialEq)]
pub struct GlideKeyboard {
    /// `a`..=`z` 各键的中心，按这个次序。
    pub centers: [(f32, f32); 26],
    pub key_width: f32,
    pub key_height: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlideHypothesis {
    /// 小写全拼字母，不带分隔符，每个音节都完整。
    pub letters: String,
    pub cost: f32,
}

#[derive(Debug, Clone, Copy)]
struct Sample {
    x: f32,
    y: f32,
    dwell: bool,
}

impl GlideKeyboard {
    /// 这套键位能不能用来解码：键宽键高为正的有限数，键中心都有限。
    pub fn is_valid(&self) -> bool {
        self.key_width.is_finite()
            && self.key_height.is_finite()
            && self.key_width > 0.0
            && self.key_height > 0.0
            && self
                .centers
                .iter()
                .all(|(x, y)| x.is_finite() && y.is_finite())
    }

    fn normalized_centers(&self) -> [(f32, f32); 26] {
        self.centers
            .map(|(x, y)| (x / self.key_width, y / self.key_height))
    }
}

/// 这一笔至多 `limit` 个字母串，代价低的在前；键位无效、不足两个点、有非有限坐标，或者没有完整的全拼串跟得上这一笔时为空。
pub fn decode_glide(
    keyboard: &GlideKeyboard,
    points: &[GlidePoint],
    limit: usize,
) -> Vec<GlideHypothesis> {
    if limit == 0
        || points.len() < 2
        || !keyboard.is_valid()
        || points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite())
    {
        return Vec::new();
    }
    let normalized: Vec<GlidePoint> = points
        .iter()
        .map(|p| GlidePoint {
            x: p.x / keyboard.key_width,
            y: p.y / keyboard.key_height,
            time_ms: p.time_ms,
        })
        .collect();
    let samples = resample(&normalized);
    if samples.len() < 2 {
        return Vec::new();
    }
    Decoder::new(keyboard.normalized_centers(), &samples).run(limit)
}

/// `points` 里各次停留的弧长位置：手指在 [`DWELL_RADIUS`] 以内停了 [`DWELL_MS`] 或更久的地方。
fn dwell_positions(points: &[GlidePoint], arc: &[f32]) -> Vec<f32> {
    let mut dwells = Vec::new();
    let mut start = 0;
    while start < points.len() {
        let Some(begin) = points[start].time_ms else {
            start += 1;
            continue;
        };
        let mut end = start;
        while end + 1 < points.len() && distance(points[start], points[end + 1]) <= DWELL_RADIUS {
            end += 1;
        }
        let rested = points[end]
            .time_ms
            .is_some_and(|stop| stop.saturating_sub(begin) >= DWELL_MS);
        if rested {
            dwells.push((arc[start] + arc[end]) / 2.0);
            start = end + 1;
        } else {
            start += 1;
        }
    }
    dwells
}

fn distance(a: GlidePoint, b: GlidePoint) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

/// 沿笔画等距的采样点，含两端，离每次停留最近的采样点做上标记。
fn resample(points: &[GlidePoint]) -> Vec<Sample> {
    let mut arc = Vec::with_capacity(points.len());
    let mut length = 0.0;
    arc.push(0.0);
    for pair in points.windows(2) {
        length += distance(pair[0], pair[1]);
        arc.push(length);
    }
    let step = SAMPLE_STEP.max(length / (MAX_SAMPLES - 1) as f32);
    let count = ((length / step).ceil() as usize + 1).clamp(2, MAX_SAMPLES);
    let mut samples = Vec::with_capacity(count);
    let mut segment = 0;
    for index in 0..count {
        let target = length * index as f32 / (count - 1) as f32;
        while segment + 2 < arc.len() && arc[segment + 1] < target {
            segment += 1;
        }
        let (a, b) = (points[segment], points[segment + 1]);
        let span = arc[segment + 1] - arc[segment];
        let t = if span > 0.0 {
            ((target - arc[segment]) / span).clamp(0.0, 1.0)
        } else {
            0.0
        };
        samples.push(Sample {
            x: a.x + (b.x - a.x) * t,
            y: a.y + (b.y - a.y) * t,
            dwell: false,
        });
    }
    if length > 0.0 {
        for dwell in dwell_positions(points, &arc) {
            let index = ((dwell / length) * (count - 1) as f32).round() as usize;
            samples[index.min(count - 1)].dwell = true;
        }
    }
    samples
}

/// 完整音节组成的字母前缀树。节点 0 是根，也代表一个恰好结束在两个音节之间的前缀。
struct SyllableTrie {
    /// 每个字母的子节点；0 表示没有，因为根不会是谁的子节点。
    children: Vec<[u16; 26]>,
    intact: Vec<bool>,
    /// 节点有字母 `l` 的子节点时第 `l` 位为 1。
    masks: Vec<u32>,
}

fn syllable_trie() -> &'static SyllableTrie {
    static TRIE: OnceLock<SyllableTrie> = OnceLock::new();
    TRIE.get_or_init(|| {
        let mut trie = SyllableTrie {
            children: vec![[0; 26]],
            intact: vec![false],
            masks: Vec::new(),
        };
        for syllable in intact_pinyin_list() {
            let mut node = 0;
            for &letter in syllable.as_bytes() {
                let slot = usize::from(letter - b'a');
                if trie.children[node][slot] == 0 {
                    trie.children.push([0; 26]);
                    trie.intact.push(false);
                    trie.children[node][slot] =
                        u16::try_from(trie.children.len() - 1).expect("the syllable trie is small");
                }
                node = usize::from(trie.children[node][slot]);
            }
            trie.intact[node] = true;
        }
        trie.masks = trie
            .children
            .iter()
            .map(|children| {
                (0..26)
                    .filter(|&letter| children[letter] != 0)
                    .fold(0, |mask, letter| mask | 1 << letter)
            })
            .collect();
        trie
    })
}

impl SyllableTrie {
    /// `states` 里某种切分后面可以接的字母。
    fn followers(&self, states: &[u16]) -> u32 {
        states
            .iter()
            .fold(0, |mask, &state| mask | self.masks[usize::from(state)])
    }

    /// 把在 `states` 所示切分的前缀后面接上 `letter` 之后、每种合法切分的状态写进 `next`：音节还能长时是长出来的那个音节，音节完整时是根。没有合法切分时 `next` 为空。
    fn extend(&self, states: &[u16], letter: u8, next: &mut Vec<u16>) {
        next.clear();
        for &state in states {
            let child = self.children[usize::from(state)][usize::from(letter)];
            if child == 0 {
                continue;
            }
            let node = usize::from(child);
            if self.intact[node] && !next.contains(&0) {
                next.push(0);
            }
            if self.children[node].iter().any(|&grand| grand != 0) && !next.contains(&child) {
                next.push(child);
            }
        }
    }
}

/// 一个字母前缀（字母记作 `0..26`）、它各种合法切分在前缀树里的状态，以及它对齐到各采样点的代价，按采样点排序。只保留剪枝后留下的采样点，所以一个前缀只有寥寥几项，而不是每个采样点一项。
struct Prefix {
    letters: Vec<u8>,
    states: Vec<u16>,
    costs: Vec<(usize, f32)>,
}

struct Decoder {
    /// 把字母 `l` 对齐到采样点 `j` 的代价是 `align[l][j]`；笔画从没靠近的字母为 `None`。
    align: [Option<Vec<f32>>; 26],
    /// 离各字母键 [`NEAR_DISTANCE`] 以内的采样点，升序：字母只对齐到这些地方。
    reach: [Vec<usize>; 26],
    /// 字母对 `from * 26 + to` 的逐点累计代价：偏离两键连线、沿连线往回走、跳过停留；第 `j` 项覆盖采样点 `0..=j`。只为两个字母都靠近笔画的字母对填写。
    lines: Vec<Option<Vec<f32>>>,
    count: usize,
}

impl Decoder {
    fn new(centers: [(f32, f32); 26], samples: &[Sample]) -> Self {
        let length: f32 = samples
            .windows(2)
            .map(|pair| ((pair[0].x - pair[1].x).powi(2) + (pair[0].y - pair[1].y).powi(2)).sqrt())
            .sum();
        // 每个采样点代表的笔画长度，这样连线代价与采样疏密无关。
        let weight = length / (samples.len() - 1) as f32;
        let near = NEAR_DISTANCE * NEAR_DISTANCE;
        let squared: [Vec<f32>; 26] = std::array::from_fn(|letter| {
            let (cx, cy) = centers[letter];
            samples
                .iter()
                .map(|s| (s.x - cx).powi(2) + (s.y - cy).powi(2))
                .collect()
        });
        let reach: [Vec<usize>; 26] = std::array::from_fn(|letter| {
            (0..samples.len())
                .filter(|&j| squared[letter][j] <= near)
                .collect()
        });
        let align: [Option<Vec<f32>>; 26] = std::array::from_fn(|letter| {
            (!reach[letter].is_empty())
                .then(|| squared[letter].iter().map(|d| d / ALIGN_SCALE).collect())
        });
        let mut lines = vec![None; 26 * 26];
        for from in 0..26 {
            for to in 0..26 {
                if align[from].is_none() || align[to].is_none() {
                    continue;
                }
                let a = centers[from];
                let b = centers[to];
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let span = dx * dx + dy * dy;
                let length = span.sqrt();
                let mut total = 0.0;
                let mut previous: Option<&Sample> = None;
                lines[from * 26 + to] = Some(
                    samples
                        .iter()
                        .map(|s| {
                            if let Some(p) = previous.filter(|_| length > 0.0) {
                                let along = ((s.x - p.x) * dx + (s.y - p.y) * dy) / length;
                                total += (-along).max(0.0) * BACKTRACK_COST;
                            }
                            previous = Some(s);
                            let t = if span > 0.0 {
                                (((s.x - a.0) * dx + (s.y - a.1) * dy) / span).clamp(0.0, 1.0)
                            } else {
                                0.0
                            };
                            let off = (s.x - a.0 - dx * t).powi(2) + (s.y - a.1 - dy * t).powi(2);
                            total += off / LINE_SCALE * weight;
                            if s.dwell {
                                total += SKIPPED_DWELL_COST;
                            }
                            total
                        })
                        .collect(),
                );
            }
        }
        Self {
            align,
            reach,
            lines,
            count: samples.len(),
        }
    }

    fn run(self, limit: usize) -> Vec<GlideHypothesis> {
        let trie = syllable_trie();
        let count = self.count;
        let last = count - 1;
        let near: Vec<u8> = (0..26u8)
            .filter(|&letter| self.align[usize::from(letter)].is_some())
            .collect();
        let mut states = Vec::new();
        let mut level: Vec<Prefix> = Vec::new();
        for &letter in &near {
            // 第一个字母在第一个采样点上。
            if self.reach[usize::from(letter)].first() != Some(&0) {
                continue;
            }
            trie.extend(&[0], letter, &mut states);
            if states.is_empty() {
                continue;
            }
            let cost = self.align[usize::from(letter)].as_ref().expect("near")[0] + LETTER_COST;
            level.push(Prefix {
                letters: vec![letter],
                states: states.clone(),
                costs: vec![(0, cost)],
            });
        }
        let mut finals: Vec<GlideHypothesis> = Vec::new();
        let mut floors = vec![f32::INFINITY; count];
        for _ in 1..MAX_GLIDE_LETTERS {
            floors.fill(f32::INFINITY);
            let mut next = Vec::new();
            for prefix in &level {
                let from = usize::from(*prefix.letters.last().expect("prefixes are never empty"));
                let followers = trie.followers(&prefix.states);
                let earliest = prefix.costs[0].0;
                for &letter in &near {
                    // 只扩展音节允许接在后面、并且键位在前缀最早对齐点之后还会被经过的字母。
                    if followers & 1 << letter == 0
                        || self.reach[usize::from(letter)].last() <= Some(&earliest)
                    {
                        continue;
                    }
                    trie.extend(&prefix.states, letter, &mut states);
                    if states.is_empty() {
                        continue;
                    }
                    let lines = self.lines[from * 26 + usize::from(letter)]
                        .as_deref()
                        .expect("lines are filled for every pair of near letters");
                    let align = self.align[usize::from(letter)].as_deref().expect("near");
                    let step = if usize::from(letter) == from {
                        LETTER_COST + REPEAT_COST
                    } else {
                        LETTER_COST
                    };
                    // `best` 是所有对齐点 i < j 上 cost[i] - lines[i] 的最小值；i 与 j 之间的采样点再付 lines[j - 1] - lines[i]。
                    let mut best = f32::INFINITY;
                    let mut taken = 0;
                    let mut costs = Vec::new();
                    for &j in &self.reach[usize::from(letter)] {
                        while taken < prefix.costs.len() && prefix.costs[taken].0 < j {
                            let (i, cost) = prefix.costs[taken];
                            best = best.min(cost - lines[i]);
                            taken += 1;
                        }
                        if !best.is_finite() {
                            continue;
                        }
                        let cost = best + lines[j - 1] + align[j] + step;
                        if cost <= floors[j] + BEAM {
                            floors[j] = floors[j].min(cost);
                            costs.push((j, cost));
                        }
                    }
                    if costs.is_empty() {
                        continue;
                    }
                    let mut letters = Vec::with_capacity(prefix.letters.len() + 1);
                    letters.extend_from_slice(&prefix.letters);
                    letters.push(letter);
                    next.push(Prefix {
                        letters,
                        states: states.clone(),
                        costs,
                    });
                }
            }
            prune(&mut next, count);
            finals.extend(next.iter().filter_map(|prefix| {
                let &(at, cost) = prefix.costs.last()?;
                (at == last && prefix.states.contains(&0)).then(|| GlideHypothesis {
                    letters: prefix
                        .letters
                        .iter()
                        .map(|&l| char::from(b'a' + l))
                        .collect(),
                    cost,
                })
            }));
            // 字母带来的每一项代价都不为负，所以已经和第 `limit` 个找到的串一样贵的前缀只会排在它后面。
            if finals.len() >= limit {
                finals.select_nth_unstable_by(limit - 1, |a, b| a.cost.total_cmp(&b.cost));
                finals.truncate(limit);
                let ceiling = finals
                    .iter()
                    .map(|hypothesis| hypothesis.cost)
                    .fold(f32::NEG_INFINITY, f32::max);
                for prefix in &mut next {
                    prefix.costs.retain(|&(_, cost)| cost < ceiling);
                }
                next.retain(|prefix| !prefix.costs.is_empty());
            }
            if next.is_empty() {
                break;
            }
            level = next;
        }
        finals.sort_by(|a, b| {
            a.cost
                .total_cmp(&b.cost)
                .then_with(|| a.letters.cmp(&b.letters))
        });
        finals.truncate(limit);
        finals
    }
}

/// 在每个采样点上只保留 [`TOKENS_PER_SAMPLE`] 个最便宜、且与最便宜者相差不超过 [`BEAM`] 的前缀；哪里都没保留的前缀丢掉。
fn prune(prefixes: &mut Vec<Prefix>, count: usize) {
    let mut at: Vec<Vec<(f32, usize, usize)>> = vec![Vec::new(); count];
    for (p, prefix) in prefixes.iter().enumerate() {
        for (entry, &(j, cost)) in prefix.costs.iter().enumerate() {
            at[j].push((cost, p, entry));
        }
    }
    let mut keep: Vec<Vec<bool>> = prefixes
        .iter()
        .map(|prefix| vec![false; prefix.costs.len()])
        .collect();
    for tokens in &mut at {
        if tokens.len() > TOKENS_PER_SAMPLE {
            tokens.select_nth_unstable_by(TOKENS_PER_SAMPLE - 1, |a, b| a.0.total_cmp(&b.0));
            tokens.truncate(TOKENS_PER_SAMPLE);
        }
        let floor = tokens
            .iter()
            .map(|token| token.0)
            .fold(f32::INFINITY, f32::min);
        for &(cost, p, entry) in tokens.iter() {
            if cost <= floor + BEAM {
                keep[p][entry] = true;
            }
        }
    }
    for (prefix, keep) in prefixes.iter_mut().zip(&keep) {
        let mut flags = keep.iter();
        prefix
            .costs
            .retain(|_| *flags.next().expect("one flag per entry"));
    }
    prefixes.retain(|prefix| !prefix.costs.is_empty());
}

/// 把 `letters` 切成完整音节的各种切法，至多 `limit` 种。
fn syllable_splits(letters: &str, limit: usize) -> Vec<Vec<&str>> {
    fn walk<'a>(
        rest: &'a str,
        split: &mut Vec<&'a str>,
        found: &mut Vec<Vec<&'a str>>,
        limit: usize,
    ) {
        if found.len() >= limit {
            return;
        }
        if rest.is_empty() {
            found.push(split.clone());
            return;
        }
        for end in (1..=rest.len().min(MAX_SYLLABLE_LENGTH)).rev() {
            if is_intact(&rest[..end]) {
                split.push(&rest[..end]);
                walk(&rest[end..], split, found, limit);
                split.pop();
            }
        }
    }
    let mut found = Vec::new();
    walk(letters, &mut Vec::new(), &mut found, limit);
    found
}

/// 按笔画与串的吻合程度和词典给出的常用程度重排解码出的串。
///
/// `weights` 对一组词典键（以 `'` 连接的音节）回答它认得的每个键下最好那一行的权重。一个串按它是词的切分里权重最高的那种计分；完全不是词的串按最罕见的音节计分再减 [`UNKNOWN_WORD_COST`]，所以笔画吻合程度相当时，词典词总排在一串零散音节前面。
pub fn rank_by_dictionary(
    hypotheses: Vec<GlideHypothesis>,
    weights: impl FnOnce(&[String]) -> HashMap<String, i64>,
) -> Vec<GlideHypothesis> {
    const SPLITS: usize = 8;
    let splits: Vec<Vec<Vec<&str>>> = hypotheses
        .iter()
        .map(|hypothesis| syllable_splits(&hypothesis.letters, SPLITS))
        .collect();
    let mut keys: Vec<String> = Vec::new();
    for split in splits.iter().flatten() {
        keys.push(split.join("'"));
        keys.extend(split.iter().map(|syllable| (*syllable).to_owned()));
    }
    keys.sort();
    keys.dedup();
    let known = weights(&keys);
    let log_weight = |key: &str| known.get(key).map(|&weight| (weight.max(0) as f32).ln_1p());
    let scores: Vec<f32> = splits
        .iter()
        .map(|splits| {
            let word = splits
                .iter()
                .filter_map(|split| {
                    log_weight(&split.join("'"))
                        .map(|weight| weight + WORD_SYLLABLE_BONUS * (split.len() - 1) as f32)
                })
                .fold(None, |best: Option<f32>, weight| {
                    Some(best.map_or(weight, |b| b.max(weight)))
                });
            word.unwrap_or_else(|| {
                splits
                    .iter()
                    .map(|split| {
                        split
                            .iter()
                            .map(|syllable| log_weight(syllable).unwrap_or(0.0))
                            .fold(f32::INFINITY, f32::min)
                    })
                    .fold(0.0, f32::max)
                    - UNKNOWN_WORD_COST
            })
        })
        .collect();
    let mut ranked: Vec<GlideHypothesis> = hypotheses
        .into_iter()
        .zip(scores)
        .map(|(hypothesis, score)| GlideHypothesis {
            cost: hypothesis.cost - DICTIONARY_WEIGHT * score,
            ..hypothesis
        })
        .collect();
    ranked.sort_by(|a, b| {
        a.cost
            .total_cmp(&b.cost)
            .then_with(|| a.letters.cmp(&b.letters))
    });
    ranked
}

#[cfg(test)]
pub(crate) mod tests;
