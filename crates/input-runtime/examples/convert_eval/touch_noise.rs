//! 触屏误触的合成：在参考键位几何上，围绕想按的键心按高斯分布撒落点，按矩形命中得到实际按到的键，评测把这个键当作用户按下的键送进去。`convert_eval` 和 `rerank_latency` 共用。
//!
//! 落点服从 N(键心 + (0, b), σ²)，σ 和 b 的单位是键：横向按键宽的中位数、纵向按键高的中位数归一，与引擎将来归一宿主几何的做法一致。b 为正表示落点普遍偏下（屏幕坐标 y 向下）。落在某个键的矩形里就是按到了它；落在键距或字母区外时归最近的键，模拟宿主把键距让给相邻键的路由。字母区外的功能键（大小写、删除、候选栏）不建模，落在那里的点也归最近的字母键，所以按错率偏低。
//!
//! 分布用网格积分一次算好：以瞄准点为中心、±5σ 的方格按 σ/8 划格，每格取格心的密度为权，按格心命中的键归类后归一。每个想按的键因此有一行「按到各键的概率」，每个按到的键下面挂着落在它上面的那些格。抽一键只用一个均匀随机数：先按概率选出按到的键，剩下的那一截再在这个键的格里按权选出落点（格心），送给引擎当按下点。这样「这一键按错」「这一键按对」这种条件分布也能直接抽，σ 很小时不需要反复重抽；按到哪个键与只抽键时用的是同一个随机数、同一次比较，所以同一个种子抽出的键和引擎收不到触点的那一期（平面表基线）逐个相同。

use msime_engine::ShuangpinProfileKind;
use rand::{RngExt, SeedableRng};
use rand_pcg::Pcg64;
use serde::Deserialize;
use std::path::Path;

/// 落点，宿主坐标（与几何文件同一个坐标系）。
pub type Point = (f64, f64);

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
    /// 文件原样的字节，原样交给引擎注册（`msime_engine::touch::KeyGeometry::from_json`）。
    pub json: Vec<u8>,
    /// 先是 a 到 z，有 `;` 时接着是它，最后是重复键；同一个字母取第一个实例作为瞄准的键。
    keys: Vec<KeyRect>,
    unit_w: f64,
    unit_h: f64,
}

impl KeyLayout {
    pub fn load(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let json = std::fs::read(path)?;
        let file: GeometryFile = serde_json::from_slice(&json)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or("geometry file name is not UTF-8")?
            .to_owned();
        let mut layout =
            Self::from_file(name, file).map_err(|error| format!("{}: {error}", path.display()))?;
        layout.json = json;
        Ok(layout)
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
            json: Vec::new(),
            keys,
            unit_w,
            unit_h,
        })
    }

    /// 这份几何里有没有 `;` 键。宿主只在方案用 `;` 键时（内置方案里只有微软双拼）显示它，第二行因此多一个键、键宽也不同，所以这种方案要用自己的那份几何，见 [`layout_for`]。
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

/// 26 键双拼的内置方案；`custom` 没有内置的表，评测不了。
const SHUANGPIN_PROFILES: [ShuangpinProfileKind; 5] = [
    ShuangpinProfileKind::Xiaohe,
    ShuangpinProfileKind::Ziranma,
    ShuangpinProfileKind::Shoudao,
    ShuangpinProfileKind::Microsoft,
    ShuangpinProfileKind::SmartAbc,
];

/// 按方案名取一个内置 26 键双拼方案，`convert_eval` 和 `rerank_latency` 的 `--shuangpin-profile` 共用。
pub fn parse_profile(name: &str) -> Result<ShuangpinProfileKind, String> {
    SHUANGPIN_PROFILES
        .into_iter()
        .find(|kind| kind.name() == name)
        .ok_or_else(|| {
            format!("unknown shuangpin profile: {name} (expected xiaohe, ziranma, shoudao, microsoft or smart_abc)")
        })
}

/// 方案在 [`case_rng`] 标签里的值。写死而不取 `ShuangpinProfileKind` 的序号：引擎改了序号，已提交基线的随机数流也不能跟着变。这些值就是生成基线时的序号，`seed_labels_are_pinned` 钉住它们。
// rerank_latency 也编进这个模块，但它只跑一组配置，随机数流只按用例序号分，不用这个标签。
#[allow(dead_code)]
pub fn profile_label(profile: ShuangpinProfileKind) -> u64 {
    match profile {
        ShuangpinProfileKind::Xiaohe => 0,
        ShuangpinProfileKind::Ziranma => 1,
        ShuangpinProfileKind::Shoudao => 2,
        ShuangpinProfileKind::Microsoft => 3,
        ShuangpinProfileKind::Custom => 4,
        ShuangpinProfileKind::SmartAbc => 5,
    }
}

/// `layouts` 里 `profile` 该用的那一份的下标。宿主只在方案用 `;` 键时显示它（判据是引擎的 `shuangpin_uses_semicolon`，与会话决定显不显示 `;` 的是同一条），第二行因此多一个键，所以用 `;` 的方案取带 `;` 的那份，其余方案取不带的。同一种给了不止一份时说不清该用哪份，报错而不是默默取第一份。
pub fn layout_for(layouts: &[KeyLayout], profile: ShuangpinProfileKind) -> Result<usize, String> {
    let semicolon = msime_engine::host::shuangpin_uses_semicolon(profile.name())
        .ok_or_else(|| format!("{} has no built-in table", profile.name()))?;
    let with = if semicolon { "with" } else { "without" };
    let matching: Vec<usize> = (0..layouts.len())
        .filter(|&index| layouts[index].has_semicolon() == semicolon)
        .collect();
    match matching.as_slice() {
        [index] => Ok(*index),
        [] => Err(format!(
            "{} needs a --key-geometry {with} a ; key",
            profile.name()
        )),
        several => Err(format!(
            "{} matches several --key-geometry files {with} a ; key ({}); give one geometry {with} a ; key and at most one other",
            profile.name(),
            several
                .iter()
                .map(|&index| layouts[index].name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// 从 `--key-geometry` 给的几何里留下 `profiles` 实际用上的那几份，按给出的顺序；报告的 `geometry` 照它列。带 `;` 和不带 `;` 的各最多给一份，同一种给了不止一份时先报错，不看这次选的方案用不用得上它、也不看是不是只跑 none：几何能不能用不该取决于这次跑了哪些方案。`touches` 为假（只跑 none）时不按键位合成误触，一份都用不上，返回空。
pub fn select_layouts(
    mut layouts: Vec<KeyLayout>,
    profiles: &[ShuangpinProfileKind],
    touches: bool,
) -> Result<Vec<KeyLayout>, String> {
    for semicolon in [false, true] {
        let names: Vec<&str> = layouts
            .iter()
            .filter(|layout| layout.has_semicolon() == semicolon)
            .map(|layout| layout.name.as_str())
            .collect();
        if names.len() > 1 {
            let with = if semicolon { "with" } else { "without" };
            return Err(format!(
                "several --key-geometry files {with} a ; key ({}); give at most one geometry with a ; key and one without",
                names.join(", ")
            ));
        }
    }
    let mut used = vec![false; layouts.len()];
    if touches {
        for profile in profiles {
            used[layout_for(&layouts, *profile)?] = true;
        }
    }
    let mut used = used.into_iter();
    layouts.retain(|_| used.next().unwrap_or(false));
    Ok(layouts)
}

/// `--touch-sigma` 的一个值：有限、不小于 0。
pub fn parse_sigma(text: &str) -> Result<f64, String> {
    parse_finite(text)
        .filter(|value| *value >= 0.0)
        .ok_or_else(|| format!("--touch-sigma: {text} is not a finite non-negative number"))
}

/// `--touch-bias-y` 的一个值：有限，可以为负（落点普遍偏上）。
pub fn parse_bias_y(text: &str) -> Result<f64, String> {
    parse_finite(text).ok_or_else(|| format!("--touch-bias-y: {text} is not a finite number"))
}

/// 有限的数；`-0` 记成 0，否则它会以 `-0.0` 写进报告，随机数流的标签（按位取值）也和 0 不同。
fn parse_finite(text: &str) -> Option<f64> {
    let value = text.parse::<f64>().ok().filter(|value| value.is_finite())?;
    Some(if value == 0.0 { 0.0 } else { value })
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

    /// 模式在 [`case_rng`] 标签里的值。写死而不取变体的序号，调换变体的顺序不会改变已提交基线的随机数流；这些值就是生成基线时的序号。
    // rerank_latency 也编进这个模块，但它只跑一组配置，随机数流只按用例序号分，不用这个标签。
    #[allow(dead_code)]
    pub fn label(self) -> u64 {
        match self {
            Self::None => 0,
            Self::Clean => 1,
            Self::Noisy => 2,
            Self::Realistic => 3,
        }
    }
}

/// 一份几何在一组 (σ, b) 下，每个想按的键按到各键的概率，以及落在各键上的那些格。
pub struct TouchNoise {
    /// 按想按的键排列：(想按的键, 瞄准点, 按到的各键)；按到的键按键排序、概率和为 1。
    rows: Vec<(u8, Point, Vec<Hit>)>,
}

/// 想按某个键时按到 `key` 的那一部分：概率，以及落在 `key` 上的格（宿主坐标的格心和归一后的权，权的和就是 `p`）。
struct Hit {
    key: u8,
    p: f64,
    cells: Vec<(Point, f64)>,
}

impl Hit {
    /// 这个键里累计权刚超过 `target` 的那一格的格心；浮点尾差落在最后一格。
    fn point(&self, mut target: f64) -> Point {
        for &(point, weight) in &self.cells {
            if target < weight {
                return point;
            }
            target -= weight;
        }
        self.cells.last().map_or((0.0, 0.0), |(point, _)| *point)
    }
}

/// 抽一键时只在哪些按到的键里抽。
#[derive(Clone, Copy, PartialEq)]
enum Pick {
    Any,
    /// 只在按错的键里抽（noisy 的那一键）。
    Miss,
    /// 只在按对的格里抽（clean）。
    Hit,
}

/// 积分方格的半径，以 σ 计。
const WINDOW_SIGMAS: f64 = 5.0;
/// 每个 σ 划几格。
const CELLS_PER_SIGMA: f64 = 8.0;

impl TouchNoise {
    pub fn new(layout: &KeyLayout, sigma: f64, bias_y: f64) -> Self {
        let mut rows = Vec::new();
        for aimed in &layout.keys {
            if rows.iter().any(|(key, _, _)| *key == aimed.key) {
                continue;
            }
            // 归一后的瞄准点；网格在归一坐标里划，命中测试换回宿主坐标做。
            let (u0, v0) = (aimed.cx / layout.unit_w, aimed.cy / layout.unit_h + bias_y);
            let mut mass: Vec<Hit> = Vec::new();
            let mut add = |point: Point, weight: f64| {
                let key = layout.hit(point.0, point.1);
                match mass.iter_mut().find(|hit| hit.key == key) {
                    Some(hit) => {
                        hit.p += weight;
                        hit.cells.push((point, weight));
                    }
                    None => mass.push(Hit {
                        key,
                        p: weight,
                        cells: vec![(point, weight)],
                    }),
                }
            };
            if sigma == 0.0 {
                add((u0 * layout.unit_w, v0 * layout.unit_h), 1.0);
            } else {
                let step = sigma / CELLS_PER_SIGMA;
                let half = (WINDOW_SIGMAS * CELLS_PER_SIGMA) as i32;
                for row in -half..=half {
                    for column in -half..=half {
                        let (du, dv) = (f64::from(column) * step, f64::from(row) * step);
                        let weight = (-(du * du + dv * dv) / (2.0 * sigma * sigma)).exp();
                        add(
                            ((u0 + du) * layout.unit_w, (v0 + dv) * layout.unit_h),
                            weight,
                        );
                    }
                }
            }
            let total: f64 = mass.iter().map(|hit| hit.p).sum();
            for hit in &mut mass {
                hit.p /= total;
                for (_, weight) in &mut hit.cells {
                    *weight /= total;
                }
            }
            mass.sort_by_key(|hit| hit.key);
            rows.push((aimed.key, (u0 * layout.unit_w, v0 * layout.unit_h), mass));
        }
        Self { rows }
    }

    fn row(&self, key: u8) -> Result<&[Hit], String> {
        self.rows
            .iter()
            .find(|(aimed, _, _)| *aimed == key)
            .map(|(_, _, row)| row.as_slice())
            .ok_or_else(|| format!("the geometry has no {:?} key", key as char))
    }

    /// 想按 `key` 却按到别的键的概率。
    pub fn miss_probability(&self, key: u8) -> Result<f64, String> {
        Ok(self
            .row(key)?
            .iter()
            .filter(|hit| hit.key != key)
            .map(|hit| hit.p)
            .sum())
    }

    /// 想按 `key` 时按到的键和落点，`pick` 限定在哪些按到的键里按概率抽。只用一个均匀随机数。
    fn draw(&self, key: u8, pick: Pick, rng: &mut Pcg64) -> Result<(u8, Point), String> {
        let row = self.row(key)?;
        let eligible = || {
            row.iter().filter(move |hit| match pick {
                Pick::Any => true,
                Pick::Miss => hit.key != key,
                Pick::Hit => hit.key == key,
            })
        };
        let total: f64 = eligible().map(|hit| hit.p).sum();
        let mut target = rng.random::<f64>() * total;
        let mut last = None;
        for hit in eligible() {
            if target < hit.p {
                return Ok((hit.key, hit.point(target)));
            }
            target -= hit.p;
            last = Some(hit);
        }
        // 浮点累加的尾差落在最后一格。
        let hit = last.ok_or_else(|| format!("no touch can land for {:?}", key as char))?;
        Ok((hit.key, hit.point(hit.p)))
    }

    /// 一条用例在 `mode` 下实际按到的键，不要落点，见 [`Self::synthesize_touches`]。测试用它核对按到的键与只抽键时一致。
    #[cfg(test)]
    pub fn synthesize(
        &self,
        mode: TouchMode,
        intended: &[u8],
        rng: &mut Pcg64,
    ) -> Result<Option<Vec<u8>>, String> {
        Ok(self
            .synthesize_touches(mode, intended, rng)?
            .map(|touches| touches.into_iter().map(|(key, _)| key).collect()))
    }

    /// 一条用例在 `mode` 下实际按到的键和每一键的落点。`none` 的落点是瞄准点，评测不把它送给引擎；`clean` 每键都在按对的格里抽。`noisy` 在这条用例的每个键都不可能按错时（σ 为 0，或者概率小到积分里为 0）为 `None`，这条用例不进 noisy 集。
    ///
    /// noisy 按「至少一键按错」的条件分布精确抽样，不靠反复重抽：先按 P(第一个按错的是第 j 键) ∝ (∏_{i<j}(1−q_i))·q_j 选出 j，j 之前的键都按对，第 j 键在按错的键里抽，之后的键不加条件。
    pub fn synthesize_touches(
        &self,
        mode: TouchMode,
        intended: &[u8],
        rng: &mut Pcg64,
    ) -> Result<Option<Vec<(u8, Point)>>, String> {
        match mode {
            TouchMode::None => intended
                .iter()
                .map(|&key| {
                    self.rows
                        .iter()
                        .find(|(aimed, _, _)| *aimed == key)
                        .map(|&(_, aim, _)| (key, aim))
                        .ok_or_else(|| format!("the geometry has no {:?} key", key as char))
                })
                .collect::<Result<Vec<_>, _>>()
                .map(Some),
            TouchMode::Clean => intended
                .iter()
                .map(|&key| self.draw(key, Pick::Hit, rng))
                .collect::<Result<Vec<_>, _>>()
                .map(Some),
            TouchMode::Realistic => intended
                .iter()
                .map(|&key| self.draw(key, Pick::Any, rng))
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
                // j 之前的键都按对：落点在按对的格里抽，抽法与上面选 j 的随机数无关，所以放在 j 那一键之后抽，按到的键序列与只抽键时相同。
                let mut typed = Vec::with_capacity(intended.len());
                typed.push(self.draw(intended[chosen], Pick::Miss, rng)?);
                for &key in &intended[chosen + 1..] {
                    typed.push(self.draw(key, Pick::Any, rng)?);
                }
                let mut before = Vec::with_capacity(chosen + 1);
                for &key in &intended[..chosen] {
                    before.push(self.draw(key, Pick::Hit, rng)?);
                }
                before.extend(typed);
                Ok(Some(before))
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
            for (_, _, row) in &noise.rows {
                let total: f64 = row.iter().map(|hit| hit.p).sum();
                assert!((total - 1.0).abs() < 1e-9);
                for hit in row {
                    let cells: f64 = hit.cells.iter().map(|(_, weight)| weight).sum();
                    assert!((cells - hit.p).abs() < 1e-9);
                }
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
                .filter(|hit| matches!(hit.key, b's' | b'd'))
                .map(|hit| hit.p)
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
    fn touch_points_land_on_the_key_they_report() {
        let layout = test_layout(false);
        let noise = TouchNoise::new(&layout, 0.25, 0.1);
        let intended = b"xnhewovmuiuo";
        for mode in [TouchMode::Clean, TouchMode::Noisy, TouchMode::Realistic] {
            for index in 0..50 {
                let touches = noise
                    .synthesize_touches(mode, intended, &mut case_rng(3, &[index]))
                    .unwrap()
                    .unwrap();
                for &(key, (x, y)) in &touches {
                    assert_eq!(layout.hit(x, y), key, "{mode:?}");
                }
                // 只抽键的那条路径用同样的随机数流，按到的键逐个相同。
                let keys = noise
                    .synthesize(mode, intended, &mut case_rng(3, &[index]))
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    touches.iter().map(|(key, _)| *key).collect::<Vec<_>>(),
                    keys
                );
            }
        }
        // none 的落点就是瞄准点。
        let none = noise
            .synthesize_touches(TouchMode::None, b"q", &mut case_rng(3, &[]))
            .unwrap()
            .unwrap();
        let [(key, (x, y))] = none[..] else {
            panic!("{none:?}");
        };
        assert_eq!(key, b'q');
        assert!((x - 6.0).abs() < 1e-9 && (y - 7.0).abs() < 1e-9, "{none:?}");
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

    /// `seed_labels_are_pinned` 那组标签抽出的第一个数。
    const PINNED_DRAW: u64 = 5_137_228_669_518_537_305;

    /// 已提交的基线按这些标签值和这条混合规则抽样；改了其中任何一个，基线就不再能逐字节复现。
    #[test]
    fn seed_labels_are_pinned() {
        assert_eq!(SHUANGPIN_PROFILES.map(profile_label), [0, 1, 2, 3, 5]);
        assert_eq!(profile_label(ShuangpinProfileKind::Custom), 4);
        assert_eq!(TouchMode::ALL.map(TouchMode::label), [0, 1, 2, 3]);
        let labels = [
            profile_label(ShuangpinProfileKind::Microsoft),
            TouchMode::Noisy.label(),
            0.25f64.to_bits(),
            0.1f64.to_bits(),
            7,
        ];
        assert_eq!(case_rng(20261027, &labels).random::<u64>(), PINNED_DRAW);
    }

    #[test]
    fn profiles_and_layouts_follow_the_engine() {
        assert_eq!(
            parse_profile("microsoft"),
            Ok(ShuangpinProfileKind::Microsoft)
        );
        assert_eq!(
            parse_profile("smart_abc"),
            Ok(ShuangpinProfileKind::SmartAbc)
        );
        assert!(parse_profile("custom").is_err());
        assert!(parse_profile("Xiaohe").is_err());
        let layouts = [test_layout(false), test_layout(true)];
        for profile in SHUANGPIN_PROFILES {
            let expected = usize::from(profile == ShuangpinProfileKind::Microsoft);
            assert_eq!(
                layout_for(&layouts, profile),
                Ok(expected),
                "{}",
                profile.name()
            );
        }
        assert!(layout_for(&layouts, ShuangpinProfileKind::Custom).is_err());
        // 只给了一种时，另一种方案没有几何可用。
        assert!(layout_for(&layouts[..1], ShuangpinProfileKind::Microsoft).is_err());
        // 同一种给了两份，说不清该用哪份。
        let ambiguous = [test_layout(false), test_layout(true), test_layout(false)];
        let error = layout_for(&ambiguous, ShuangpinProfileKind::Xiaohe).unwrap_err();
        assert!(error.contains("several"), "{error}");
        assert_eq!(
            layout_for(&ambiguous, ShuangpinProfileKind::Microsoft),
            Ok(1)
        );
    }

    /// [`test_layout`] 换个名字，好看出留下的是哪份。
    fn named_layout(name: &str, semicolon: bool) -> KeyLayout {
        let mut layout = test_layout(semicolon);
        layout.name = name.to_owned();
        layout
    }

    #[test]
    fn only_the_layouts_the_profiles_use_are_kept_in_order() {
        let given = || {
            vec![
                named_layout("plain", false),
                named_layout("microsoft", true),
            ]
        };
        let names = |kept: Result<Vec<KeyLayout>, String>| {
            kept.unwrap()
                .into_iter()
                .map(|layout| layout.name)
                .collect::<Vec<_>>()
        };
        use ShuangpinProfileKind::{Microsoft, Xiaohe, Ziranma};
        assert_eq!(names(select_layouts(given(), &[Xiaohe], true)), ["plain"]);
        assert_eq!(
            names(select_layouts(given(), &[Xiaohe, Ziranma], true)),
            ["plain"]
        );
        assert_eq!(
            names(select_layouts(given(), &[Microsoft], true)),
            ["microsoft"]
        );
        // 按 `--key-geometry` 给出的顺序列，不按方案的顺序。
        assert_eq!(
            names(select_layouts(given(), &[Microsoft, Xiaohe], true)),
            ["plain", "microsoft"]
        );
        // 只跑 none 时不合成误触，一份都用不上。
        assert!(names(select_layouts(given(), &[Xiaohe, Microsoft], false)).is_empty());
        // 方案要的那种没给。
        assert!(select_layouts(vec![named_layout("plain", false)], &[Microsoft], true).is_err());
    }

    #[test]
    fn a_kind_of_layout_given_twice_is_rejected_even_when_unused() {
        let twice_without = || {
            vec![
                named_layout("a", false),
                named_layout("b", true),
                named_layout("c", false),
            ]
        };
        // 微软双拼只用带 `;` 的那份，不带 `;` 的给了两份照样报错；只跑 none 时也一样。
        for (profiles, touches) in [
            (&[ShuangpinProfileKind::Xiaohe][..], true),
            (&[ShuangpinProfileKind::Microsoft][..], true),
            (&[ShuangpinProfileKind::Xiaohe][..], false),
        ] {
            let error = select_layouts(twice_without(), profiles, touches)
                .err()
                .unwrap();
            assert!(error.contains("without a ; key (a, c)"), "{error}");
        }
        let twice_with = vec![named_layout("a", true), named_layout("b", true)];
        let error = select_layouts(twice_with, &[ShuangpinProfileKind::Xiaohe], false)
            .err()
            .unwrap();
        assert!(error.contains("with a ; key (a, b)"), "{error}");
    }

    #[test]
    fn touch_values_are_finite_and_sigma_is_not_negative() {
        assert_eq!(parse_sigma("0.25"), Ok(0.25));
        assert!(parse_sigma("-0.1").is_err());
        assert_eq!(parse_bias_y("-0.1"), Ok(-0.1));
        for bad in ["nan", "inf", "-inf", "", "x"] {
            assert!(parse_sigma(bad).is_err(), "{bad}");
            assert!(parse_bias_y(bad).is_err(), "{bad}");
        }
        // `-0` 记成 0：报告里不出现 -0.0，随机数流也与 0 相同。
        for value in [parse_sigma("-0").unwrap(), parse_bias_y("-0.0").unwrap()] {
            assert_eq!(value.to_bits(), 0f64.to_bits());
        }
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
