//! 触屏误触的合成：在参考键位几何上，围绕想按的键心按高斯分布撒落点，按矩形命中得到实际按到的键，评测把这个键当作用户按下的键送进去。`convert_eval` 和 `rerank_latency` 共用。
//!
//! 落点服从 N(键心 + (0, b), σ²)，σ 和 b 的单位是键：横向按键宽的中位数、纵向按键高的中位数归一，与引擎将来归一宿主几何的做法一致。b 为正表示落点普遍偏下（屏幕坐标 y 向下）。落在某个键的矩形里就是按到了它；落在键距或字母区外时归最近的键，模拟宿主把键距让给相邻键的路由。字母区外的功能键（大小写、删除、候选栏）不建模，落在那里的点也归最近的字母键，所以按错率偏低。
//!
//! 引擎现在还收不到触点，这一步只决定按到了哪个键，所以每个想按的键只需要「按到各键的概率」这一行分布。分布用网格积分一次算好：以瞄准点为中心、±5σ 的方格按 σ/8 划格，每格取格心的密度为权，按格心命中的键归类后归一。这样「这一键按错」这种条件分布也能直接抽，σ 很小时不需要反复重抽。

use rand::{RngExt, SeedableRng};
use rand_pcg::Pcg64;
use serde::Deserialize;
use std::path::Path;

/// 宿主下发给引擎的几何形状（`msime_client_set_key_geometry` 的 JSON）。坐标系由宿主自定，评测只用到相对位置。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GeometryFile {
    /// 宿主分配的代次，非零。
    id: u32,
    /// a 到 z 的 `[cx, cy, w, h]`。
    keys: Vec<[f64; 4]>,
    /// 微软双拼显示 `;` 键时它的位置，否则为 `null`。
    semicolon: Option<[f64; 4]>,
    /// 分离键盘里重复的字母键：`["g", cx, cy, w, h]`。
    duplicates: Vec<(String, f64, f64, f64, f64)>,
}

#[derive(Clone, Copy)]
struct KeyRect {
    key: u8,
    cx: f64,
    cy: f64,
    w: f64,
    h: f64,
}

impl KeyRect {
    fn new(key: u8, [cx, cy, w, h]: [f64; 4]) -> Result<Self, String> {
        if ![cx, cy, w, h].iter().all(|value| value.is_finite()) || w <= 0.0 || h <= 0.0 {
            return Err(format!(
                "key {:?}: needs finite numbers and a positive size",
                key as char
            ));
        }
        Ok(Self { key, cx, cy, w, h })
    }

    fn contains(&self, x: f64, y: f64) -> bool {
        let left = self.cx - self.w / 2.0;
        let top = self.cy - self.h / 2.0;
        x >= left && x < left + self.w && y >= top && y < top + self.h
    }

    /// 点到矩形的距离的平方，在矩形里为 0。
    fn distance_squared(&self, x: f64, y: f64) -> f64 {
        let dx = ((x - self.cx).abs() - self.w / 2.0).max(0.0);
        let dy = ((y - self.cy).abs() - self.h / 2.0).max(0.0);
        dx * dx + dy * dy
    }
}

/// 一份参考键位几何：字母键（以及微软双拼的 `;`、分离键盘的重复键）的矩形，和归一用的键宽、键高中位数。
pub struct KeyLayout {
    /// 文件名去掉扩展名，写进报告。
    pub name: String,
    /// 先是 a 到 z，有 `;` 时接着是它，最后是重复键；同一个字母取第一个实例作为瞄准的键。
    keys: Vec<KeyRect>,
    unit_w: f64,
    unit_h: f64,
}

impl KeyLayout {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let file: GeometryFile = serde_json::from_slice(&std::fs::read(path)?)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or("geometry file name is not UTF-8")?
            .to_owned();
        Self::from_file(name, file).map_err(|error| format!("{}: {error}", path.display()).into())
    }

    fn from_file(name: String, file: GeometryFile) -> Result<Self, String> {
        if file.id == 0 {
            return Err("id 0 means no geometry; a layout needs a non-zero id".into());
        }
        if file.keys.len() != 26 {
            return Err(format!(
                "expected 26 letter keys, found {}",
                file.keys.len()
            ));
        }
        let mut keys = Vec::with_capacity(27 + file.duplicates.len());
        for (key, rect) in (b'a'..=b'z').zip(file.keys) {
            keys.push(KeyRect::new(key, rect)?);
        }
        if let Some(rect) = file.semicolon {
            keys.push(KeyRect::new(b';', rect)?);
        }
        for (letter, cx, cy, w, h) in file.duplicates {
            let key = match letter.as_bytes() {
                [key @ b'a'..=b'z'] => *key,
                _ => return Err(format!("duplicate {letter:?} is not one lowercase letter")),
            };
            keys.push(KeyRect::new(key, [cx, cy, w, h])?);
        }
        let median = |mut values: Vec<f64>| {
            values.sort_by(f64::total_cmp);
            values[values.len() / 2]
        };
        let unit_w = median(keys[..26].iter().map(|key| key.w).collect());
        let unit_h = median(keys[..26].iter().map(|key| key.h).collect());
        Ok(Self {
            name,
            keys,
            unit_w,
            unit_h,
        })
    }

    /// 这份几何里有没有 `;` 键。宿主只在微软双拼下显示它，第二行因此多一个键、键宽也不同，所以微软双拼要用自己的那份几何。
    pub fn has_semicolon(&self) -> bool {
        self.keys.iter().any(|key| key.key == b';')
    }

    /// 落点 (x, y) 按到的键：落在矩形里取那个键，否则取最近的键，距离相同取靠前的。
    fn hit(&self, x: f64, y: f64) -> u8 {
        if let Some(rect) = self.keys.iter().find(|rect| rect.contains(x, y)) {
            return rect.key;
        }
        let mut best = self.keys[0];
        let mut best_distance = f64::INFINITY;
        for rect in &self.keys {
            let distance = rect.distance_squared(x, y);
            if distance < best_distance {
                best = *rect;
                best_distance = distance;
            }
        }
        best.key
    }
}

/// 一种触点模式下一条用例怎么按键（resources/eval/README.md「双拼与触屏误触」）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TouchMode {
    /// 不带触点，按想按的键原样送：桌面和实体键盘的基线。
    None,
    /// 手抖了但每键都按在本键上：精度闸。
    Clean,
    /// 至少一键按到了别的键：量召回。
    Noisy,
    /// 不加条件的落点分布：量净收益。
    Realistic,
}

impl TouchMode {
    pub const ALL: [Self; 4] = [Self::None, Self::Clean, Self::Noisy, Self::Realistic];

    pub fn parse(name: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|mode| mode.name() == name)
            .ok_or_else(|| {
                format!("unknown touch mode: {name} (expected none, clean, noisy or realistic)")
            })
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Clean => "clean",
            Self::Noisy => "noisy",
            Self::Realistic => "realistic",
        }
    }
}

/// 一份几何在一组 (σ, b) 下，每个想按的键按到各键的概率。
pub struct TouchNoise {
    /// 按想按的键排列；每行是 (按到的键, 概率)，按键排序、概率和为 1。
    rows: Vec<(u8, Vec<(u8, f64)>)>,
}

/// 积分方格的半径，以 σ 计。
const WINDOW_SIGMAS: f64 = 5.0;
/// 每个 σ 划几格。
const CELLS_PER_SIGMA: f64 = 8.0;

impl TouchNoise {
    pub fn new(layout: &KeyLayout, sigma: f64, bias_y: f64) -> Self {
        let mut rows = Vec::new();
        for aimed in &layout.keys {
            if rows.iter().any(|(key, _)| *key == aimed.key) {
                continue;
            }
            // 归一后的瞄准点；网格在归一坐标里划，命中测试换回宿主坐标做。
            let (u0, v0) = (aimed.cx / layout.unit_w, aimed.cy / layout.unit_h + bias_y);
            let mut mass: Vec<(u8, f64)> = Vec::new();
            let mut add = |key: u8, weight: f64| match mass.iter_mut().find(|(k, _)| *k == key) {
                Some((_, total)) => *total += weight,
                None => mass.push((key, weight)),
            };
            if sigma == 0.0 {
                add(layout.hit(u0 * layout.unit_w, v0 * layout.unit_h), 1.0);
            } else {
                let step = sigma / CELLS_PER_SIGMA;
                let half = (WINDOW_SIGMAS * CELLS_PER_SIGMA) as i32;
                for row in -half..=half {
                    for column in -half..=half {
                        let (du, dv) = (f64::from(column) * step, f64::from(row) * step);
                        let weight = (-(du * du + dv * dv) / (2.0 * sigma * sigma)).exp();
                        let key = layout.hit((u0 + du) * layout.unit_w, (v0 + dv) * layout.unit_h);
                        add(key, weight);
                    }
                }
            }
            let total: f64 = mass.iter().map(|(_, weight)| weight).sum();
            for (_, weight) in &mut mass {
                *weight /= total;
            }
            mass.sort_by_key(|(key, _)| *key);
            rows.push((aimed.key, mass));
        }
        Self { rows }
    }

    fn row(&self, key: u8) -> Result<&[(u8, f64)], String> {
        self.rows
            .iter()
            .find(|(aimed, _)| *aimed == key)
            .map(|(_, row)| row.as_slice())
            .ok_or_else(|| format!("the geometry has no {:?} key", key as char))
    }

    /// 想按 `key` 却按到别的键的概率。
    pub fn miss_probability(&self, key: u8) -> Result<f64, String> {
        Ok(self
            .row(key)?
            .iter()
            .filter(|(hit, _)| *hit != key)
            .map(|(_, p)| p)
            .sum())
    }

    /// 想按 `key` 时按到的键；`miss` 为真时只在按错的那些键里按概率抽。
    fn draw(&self, key: u8, miss: bool, rng: &mut Pcg64) -> Result<u8, String> {
        let row = self.row(key)?;
        let eligible = || row.iter().filter(move |(hit, _)| !miss || *hit != key);
        let total: f64 = eligible().map(|(_, p)| p).sum();
        let mut target = rng.random::<f64>() * total;
        let mut last = key;
        for &(hit, p) in eligible() {
            if target < p {
                return Ok(hit);
            }
            target -= p;
            last = hit;
        }
        // 浮点累加的尾差落在最后一格。
        Ok(last)
    }

    /// 一条用例在 `mode` 下实际按到的键。`noisy` 在这条用例的每个键都不可能按错时（σ 为 0，或者概率小到积分里为 0）为 `None`，这条用例不进 noisy 集。
    ///
    /// noisy 按「至少一键按错」的条件分布精确抽样，不靠反复重抽：先按 P(第一个按错的是第 j 键) ∝ (∏_{i<j}(1−q_i))·q_j 选出 j，j 之前的键都按对，第 j 键在按错的键里抽，之后的键不加条件。
    pub fn synthesize(
        &self,
        mode: TouchMode,
        intended: &[u8],
        rng: &mut Pcg64,
    ) -> Result<Option<Vec<u8>>, String> {
        match mode {
            TouchMode::None | TouchMode::Clean => Ok(Some(intended.to_vec())),
            TouchMode::Realistic => intended
                .iter()
                .map(|&key| self.draw(key, false, rng))
                .collect::<Result<Vec<_>, _>>()
                .map(Some),
            TouchMode::Noisy => {
                let mut first_miss = Vec::with_capacity(intended.len());
                let mut all_hit = 1.0;
                for &key in intended {
                    let miss = self.miss_probability(key)?;
                    first_miss.push(all_hit * miss);
                    all_hit *= 1.0 - miss;
                }
                let total: f64 = first_miss.iter().sum();
                if total <= 0.0 {
                    return Ok(None);
                }
                let mut target = rng.random::<f64>() * total;
                let mut chosen = first_miss.len() - 1;
                for (index, weight) in first_miss.iter().enumerate() {
                    if *weight > 0.0 && target < *weight {
                        chosen = index;
                        break;
                    }
                    target -= weight;
                }
                // 浮点尾差没选中任何一键时，取最后一个可能按错的键。
                if first_miss[chosen] <= 0.0 {
                    chosen = first_miss
                        .iter()
                        .rposition(|weight| *weight > 0.0)
                        .unwrap_or(chosen);
                }
                let mut typed = intended[..chosen].to_vec();
                typed.push(self.draw(intended[chosen], true, rng)?);
                for &key in &intended[chosen + 1..] {
                    typed.push(self.draw(key, false, rng)?);
                }
                Ok(Some(typed))
            }
        }
    }
}

/// 一条用例在一组配置下的随机数流，只由种子和标签决定。标签含用例序号，用例顺序不变时加减配置不会改变其余结果。
pub fn case_rng(seed: u64, labels: &[u64]) -> Pcg64 {
    // splitmix64 的混合步，逐个吸收标签。
    let mut state = seed;
    for label in labels {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15) ^ label;
        state = (state ^ (state >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        state = (state ^ (state >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        state ^= state >> 31;
    }
    Pcg64::seed_from_u64(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 一排十个键：键宽 10、键高 10、键距 2，第二排九个键错开半个键，`;` 可选。
    fn test_layout(semicolon: bool) -> KeyLayout {
        let rows = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];
        let mut keys = [[0.0; 4]; 26];
        for (row, letters) in rows.iter().enumerate() {
            let offset = [0.0, 6.0, 18.0][row];
            for (column, letter) in letters.bytes().enumerate() {
                keys[usize::from(letter - b'a')] = [
                    offset + 6.0 + 12.0 * column as f64,
                    6.0 + 12.0 * row as f64,
                    10.0,
                    10.0,
                ];
            }
        }
        KeyLayout::from_file(
            "test".into(),
            GeometryFile {
                id: 1,
                keys: keys.to_vec(),
                semicolon: semicolon.then_some([6.0 + 6.0 + 12.0 * 9.0, 18.0, 10.0, 10.0]),
                duplicates: Vec::new(),
            },
        )
        .unwrap()
    }

    #[test]
    fn a_touch_in_the_gap_goes_to_the_nearest_key() {
        let layout = test_layout(false);
        // q 的中心是 (6, 6)，右缘在 11；12.5 在 q 和 w 之间的键距里，离 w（左缘 13）更近。
        assert_eq!(layout.hit(6.0, 6.0), b'q');
        assert_eq!(layout.hit(11.5, 6.0), b'q');
        assert_eq!(layout.hit(12.5, 6.0), b'w');
        // 字母区外归最近的键。
        assert_eq!(layout.hit(-50.0, -50.0), b'q');
        assert!(!layout.has_semicolon());
        assert!(test_layout(true).has_semicolon());
    }

    #[test]
    fn zero_sigma_always_hits_the_aimed_key() {
        let noise = TouchNoise::new(&test_layout(true), 0.0, 0.1);
        for key in (b'a'..=b'z').chain([b';']) {
            assert_eq!(noise.miss_probability(key).unwrap(), 0.0, "{}", key as char);
        }
        let mut rng = case_rng(1, &[]);
        assert_eq!(
            noise
                .synthesize(TouchMode::Noisy, b"nihc", &mut rng)
                .unwrap(),
            None
        );
        assert_eq!(
            noise
                .synthesize(TouchMode::Realistic, b"nihc", &mut rng)
                .unwrap()
                .as_deref(),
            Some(&b"nihc"[..])
        );
    }

    #[test]
    fn rows_are_distributions_and_widen_with_sigma() {
        let layout = test_layout(false);
        let narrow = TouchNoise::new(&layout, 0.15, 0.0);
        let wide = TouchNoise::new(&layout, 0.35, 0.0);
        for noise in [&narrow, &wide] {
            for (_, row) in &noise.rows {
                let total: f64 = row.iter().map(|(_, p)| p).sum();
                assert!((total - 1.0).abs() < 1e-9);
            }
        }
        let g = |noise: &TouchNoise| noise.miss_probability(b'g').unwrap();
        assert!(g(&narrow) > 0.0 && g(&narrow) < g(&wide));
        // 偏下的落点让第一排的键更容易按到第二排。
        let biased = TouchNoise::new(&layout, 0.25, 0.1);
        let down = |noise: &TouchNoise| {
            noise
                .row(b'e')
                .unwrap()
                .iter()
                .filter(|(k, _)| matches!(k, b's' | b'd'))
                .map(|(_, p)| p)
                .sum::<f64>()
        };
        assert!(down(&biased) > down(&TouchNoise::new(&layout, 0.25, 0.0)));
    }

    #[test]
    fn noisy_cases_always_contain_a_miss_and_clean_cases_none() {
        let noise = TouchNoise::new(&test_layout(false), 0.15, 0.0);
        let intended = b"xnhe";
        for index in 0..200 {
            let mut rng = case_rng(20261027, &[index]);
            let noisy = noise
                .synthesize(TouchMode::Noisy, intended, &mut rng)
                .unwrap()
                .unwrap();
            assert_eq!(noisy.len(), intended.len());
            assert!(noisy.iter().zip(intended).any(|(a, b)| a != b));
            let clean = noise
                .synthesize(TouchMode::Clean, intended, &mut rng)
                .unwrap()
                .unwrap();
            assert_eq!(clean, intended);
        }
    }

    #[test]
    fn the_same_seed_and_labels_give_the_same_keys() {
        let noise = TouchNoise::new(&test_layout(false), 0.35, 0.1);
        let run = |labels: &[u64]| {
            noise
                .synthesize(
                    TouchMode::Realistic,
                    b"wovmuiuoxmgezk",
                    &mut case_rng(7, labels),
                )
                .unwrap()
        };
        assert_eq!(run(&[1, 2]), run(&[1, 2]));
        assert_ne!(
            (0..20).map(|i| run(&[1, i])).collect::<Vec<_>>(),
            (0..20).map(|i| run(&[2, i])).collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_layout_without_a_key_or_with_bad_numbers_is_rejected() {
        let file = |keys: Vec<[f64; 4]>, id| GeometryFile {
            id,
            keys,
            semicolon: None,
            duplicates: Vec::new(),
        };
        assert!(KeyLayout::from_file("x".into(), file(vec![[0.0, 0.0, 1.0, 1.0]; 25], 1)).is_err());
        assert!(KeyLayout::from_file("x".into(), file(vec![[0.0, 0.0, 1.0, 1.0]; 26], 0)).is_err());
        let mut keys = vec![[0.0, 0.0, 1.0, 1.0]; 26];
        keys[3][2] = 0.0;
        assert!(KeyLayout::from_file("x".into(), file(keys, 1)).is_err());
        let mut keys = vec![[0.0, 0.0, 1.0, 1.0]; 26];
        keys[3][0] = f64::NAN;
        assert!(KeyLayout::from_file("x".into(), file(keys, 1)).is_err());
        let noise = TouchNoise::new(&test_layout(false), 0.25, 0.0);
        assert!(noise.miss_probability(b';').is_err());
    }
}
