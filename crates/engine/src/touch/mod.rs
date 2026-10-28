//! 触屏按键的落点（#10018）：宿主注册的字母键几何、每次按键相对被按键中心的偏移，以及按落点给双拼纠错计价。
//!
//! 只有触屏宿主经 `msime_client_set_key_geometry`、`msime_client_touch_character` 送进来；桌面宿主和实体键盘永远不调用，所有用到它的地方在没有触点时都走原来的路径，结果逐字节不变。坐标系由宿主自定，引擎按字母键键宽、键高的中位数归一，一个键是单位大小，不论宿主的像素、密度和键距如何。
//!
//! 罚分看落点离候选键和离被按键各有多远：`pen(j) = clamp(B + s·(d²(P, c_j) − d²(P, c_k)), P_MIN, P_MAX)`，`d²` 按 `σx`、`σy` 各向异性，并按纵向偏置 `b_y` 把每个键的瞄准点往下挪。单位与词网格的边分相同（自然对数），纠错边从对数概率里扣它，解码不用改。设计和取值见 `.agents/notes/implemented/feature/2026-10-27-shuangpin-touch-correction.md`。

/// 偏移的量化单位：一个键分成这么多份。打分和缓存签名用同一个量化值，命中缓存的答案和重新计算的一致。
pub const TOUCH_STEPS_PER_KEY: f32 = 20.0;
/// 偏移钳到 ±1.5 键：更远的落点宿主不会归给这个键。
pub const TOUCH_OFFSET_LIMIT: i8 = 30;
/// 平面表里的 QWERTY 邻键离被按的键超过这么远（键为单位）时不算邻键。分离键盘上 T 和 Y、G 和 H 分在两半，平面表却把它们当邻键，宿主导出的 iPad 分离布局里分在两半的平面表邻键相距 4.6 个键以上；普通布局里平面表邻键的键心最远相距 2.37 个键（iPhone 微软双拼那份的 J 和 M：第二行多了 `;`，键距变宽）。
const FLAT_NEIGHBOR_REACH: f32 = 3.0;
/// 归一后的坐标（键心和落点）离原点最多这么多个键。宿主的坐标原点和单位任意，真实键盘用屏幕坐标也只有几百个键宽；超过它的几何（键宽极小、坐标极大）在算距离平方时会溢出成无穷，两个无穷相减得 NaN，罚分和邻键判断随之失效，所以整份拒绝。
const COORDINATE_LIMIT: f32 = 10_000.0;

/// 一次按键的落点：相对被按键中心的偏移，单位 1/20 键（[`TOUCH_STEPS_PER_KEY`]），钳到 ±[`TOUCH_OFFSET_LIMIT`]。`copy` 是这个字母的第几个实例：0 是 `keys` 里的那个，1 起依次是 `duplicates` 里同一字母的各个（分离键盘两边各有一个 G、V）。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct KeyTouch {
    pub dx: i8,
    pub dy: i8,
    pub copy: u8,
}

/// 宿主注册的一份字母键几何，按键宽、键高的中位数归一。`id` 是宿主分配的非零代次，布局一变（旋转、分离、单手、键高、微软 `;` 显隐）就换一个。
#[derive(Clone, Debug, PartialEq)]
pub struct KeyGeometry {
    id: u32,
    /// 归一用的键宽、键高（宿主单位）。
    unit: (f32, f32),
    /// 归一后的键心：先是 a 到 z，有 `;` 时接着是它，最后是重复键。
    keys: Vec<(u8, f32, f32)>,
    /// `unit` 和 `keys` 的内容指纹（[`Self::fingerprint`]），构造时算好。
    fingerprint: u64,
}

/// 宿主给的一个键：`[cx, cy, w, h]`，宿主坐标。
pub type KeyRect = [f32; 4];

/// 几何不合格的原因，整份拒绝。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyGeometryError {
    /// `id` 为 0：那是「清除几何」，不是一份几何。
    ZeroId,
    /// 某个数不是有限数，宽高不为正，或归一后的键心离原点超过 [`COORDINATE_LIMIT`] 个键。
    InvalidRect,
    /// 重复键不是一个小写字母。
    InvalidDuplicate,
}

impl std::fmt::Display for KeyGeometryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::ZeroId => "key geometry id must be non-zero",
            Self::InvalidRect => {
                "key geometry needs finite centers, positive sizes and a bounded extent"
            }
            Self::InvalidDuplicate => "key geometry duplicates must be one lowercase letter",
        })
    }
}

impl std::error::Error for KeyGeometryError {}

impl KeyGeometry {
    /// `keys` 是 a 到 z，`semicolon` 是微软双拼显示的 `;` 键，`duplicates` 是分离键盘里重复的字母键。数值都要有限、宽高为正，归一后的键心都在 [`COORDINATE_LIMIT`] 之内，否则整份拒绝。
    pub fn new(
        id: u32,
        keys: &[KeyRect; 26],
        semicolon: Option<KeyRect>,
        duplicates: &[(u8, KeyRect)],
    ) -> Result<Self, KeyGeometryError> {
        if id == 0 {
            return Err(KeyGeometryError::ZeroId);
        }
        let valid = |[cx, cy, w, h]: KeyRect| {
            cx.is_finite() && cy.is_finite() && w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0
        };
        if !keys
            .iter()
            .chain(semicolon.iter())
            .chain(duplicates.iter().map(|(_, rect)| rect))
            .all(|rect| valid(*rect))
        {
            return Err(KeyGeometryError::InvalidRect);
        }
        if duplicates
            .iter()
            .any(|(letter, _)| !letter.is_ascii_lowercase())
        {
            return Err(KeyGeometryError::InvalidDuplicate);
        }
        let median = |mut values: [f32; 26]| {
            values.sort_by(f32::total_cmp);
            values[values.len() / 2]
        };
        let unit = (
            median(keys.map(|[_, _, w, _]| w)),
            median(keys.map(|[_, _, _, h]| h)),
        );
        let normalized = |key: u8, [cx, cy, _, _]: KeyRect| (key, cx / unit.0, cy / unit.1);
        let mut normalized_keys = Vec::with_capacity(27 + duplicates.len());
        normalized_keys.extend(
            (b'a'..=b'z')
                .zip(keys)
                .map(|(key, rect)| normalized(key, *rect)),
        );
        normalized_keys.extend(semicolon.map(|rect| normalized(b';', rect)));
        normalized_keys.extend(
            duplicates
                .iter()
                .map(|(letter, rect)| normalized(*letter, *rect)),
        );
        // 原始数值都有限，归一后仍可能溢出（键宽 1e-3、坐标 1e36），或大到距离平方溢出。
        if !normalized_keys
            .iter()
            .all(|&(_, x, y)| within_limit(x) && within_limit(y))
        {
            return Err(KeyGeometryError::InvalidRect);
        }
        let fingerprint = content_fingerprint(unit, &normalized_keys);
        Ok(Self {
            id,
            unit,
            keys: normalized_keys,
            fingerprint,
        })
    }

    /// 宿主下发的 JSON：`{"id":u32,"keys":[[cx,cy,w,h] × 26，按 a..z],"semicolon":[cx,cy,w,h]|null,"duplicates":[["g",cx,cy,w,h],…]}`，形状沿用滑行请求。空串或 `id` 为 0 表示清除，返回 `Ok(None)`；多出的字段、不是 26 个字母键或数值不合格时整份拒绝。
    pub fn from_json(bytes: &[u8]) -> Result<Option<Self>, String> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            id: u32,
            #[serde(default)]
            keys: Vec<KeyRect>,
            #[serde(default)]
            semicolon: Option<KeyRect>,
            #[serde(default)]
            duplicates: Vec<(String, f32, f32, f32, f32)>,
        }
        if bytes.is_empty() {
            return Ok(None);
        }
        let wire: Wire =
            serde_json::from_slice(bytes).map_err(|_| "invalid key geometry".to_owned())?;
        if wire.id == 0 {
            return Ok(None);
        }
        let keys: [KeyRect; 26] = wire
            .keys
            .try_into()
            .map_err(|_| "key geometry needs 26 letter keys".to_owned())?;
        let duplicates = wire
            .duplicates
            .into_iter()
            .map(|(letter, cx, cy, w, h)| match letter.as_bytes() {
                [key] => Ok((*key, [cx, cy, w, h])),
                _ => Err(KeyGeometryError::InvalidDuplicate.to_string()),
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(wire.id, &keys, wire.semicolon, &duplicates)
            .map(Some)
            .map_err(|error| error.to_string())
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    /// 几何内容（归一单位和每个键心）的指纹。宿主可能用同一个代次下发内容不同的几何，方案层把它当新几何处理；按触点计价的缓存键要带上它，才不会把旧布局算出的答案交给新布局。内容相同（`==`）的几何指纹相同。
    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    /// 有没有 `;` 键（宿主只在微软双拼下显示它）。
    pub fn has_semicolon(&self) -> bool {
        self.keys.iter().any(|(key, _, _)| *key == b';')
    }

    /// `key` 的各个实例的键心，按 `copy` 的次序。
    fn copies(&self, key: u8) -> impl Iterator<Item = (f32, f32)> + '_ {
        self.keys
            .iter()
            .filter(move |(candidate, _, _)| *candidate == key)
            .map(|&(_, x, y)| (x, y))
    }

    /// `key` 离 `point` 最近的实例的键心。
    fn nearest_copy(&self, key: u8, point: (f32, f32)) -> Option<(f32, f32)> {
        self.copies(key).min_by(|left, right| {
            distance_squared(*left, point).total_cmp(&distance_squared(*right, point))
        })
    }

    /// 宿主坐标里的按下点 (x, y) 落在 `key` 上的触点：取 `key` 离它最近的那个实例，偏移量化到 1/20 键并钳到 ±1.5 键。`key` 不在几何里（没有 `;` 键时的 `;`、不是字母）、坐标不是有限数或归一后超出 [`COORDINATE_LIMIT`] 时为 `None`，这一键按没有触点处理。大写字母按小写算。
    pub fn touch(&self, key: u8, x: f32, y: f32) -> Option<KeyTouch> {
        let point = (x / self.unit.0, y / self.unit.1);
        if !within_limit(point.0) || !within_limit(point.1) {
            return None;
        }
        let key = key.to_ascii_lowercase();
        let (copy, center) = self
            .copies(key)
            .enumerate()
            .min_by(|(_, left), (_, right)| {
                distance_squared(*left, point).total_cmp(&distance_squared(*right, point))
            })?;
        let quantize = |offset: f32| {
            (offset * TOUCH_STEPS_PER_KEY).round().clamp(
                f32::from(-TOUCH_OFFSET_LIMIT),
                f32::from(TOUCH_OFFSET_LIMIT),
            ) as i8
        };
        Some(KeyTouch {
            dx: quantize(point.0 - center.0),
            dy: quantize(point.1 - center.1),
            copy: u8::try_from(copy).ok()?,
        })
    }

    /// 触点所在的那个实例的键心，以及还原出的（量化后的）落点，都是归一坐标。
    fn pressed(&self, key: u8, touch: KeyTouch) -> Option<((f32, f32), (f32, f32))> {
        let center = self.copies(key).nth(usize::from(touch.copy))?;
        let point = (
            center.0 + f32::from(touch.dx) / TOUCH_STEPS_PER_KEY,
            center.1 + f32::from(touch.dy) / TOUCH_STEPS_PER_KEY,
        );
        Some((center, point))
    }

    /// 按在 `pressed` 上、落点是 `touch` 时，把这一键换成 `candidate` 的罚分。候选键取离落点最近的实例，所以分离键盘上的重复键各自就近。`pressed` 或 `candidate` 不在几何里时为 `None`。
    pub fn penalty(
        &self,
        pressed: u8,
        touch: KeyTouch,
        candidate: u8,
        tuning: &TouchTuning,
    ) -> Option<f64> {
        let (center, point) = self.pressed(pressed, touch)?;
        let other = self.nearest_copy(candidate, point)?;
        let spread = |(cx, cy): (f32, f32)| {
            let dx = (f64::from(point.0) - f64::from(cx)) / tuning.sigma_x;
            let dy = (f64::from(point.1) - f64::from(cy) - tuning.bias_y) / tuning.sigma_y;
            dx * dx + dy * dy
        };
        let raw = tuning.base + tuning.scale * (spread(other) - spread(center));
        Some(raw.clamp(tuning.min_penalty, tuning.max_penalty))
    }

    /// 平面表的邻键 `candidate` 在这份几何上是否真的挨着被按的那个实例。分离键盘上 T 和 Y 分在两半，平面表的这条邻接就不成立。
    pub fn adjacent(&self, pressed: u8, touch: KeyTouch, candidate: u8) -> bool {
        let Some((center, _)) = self.pressed(pressed, touch) else {
            return false;
        };
        self.nearest_copy(candidate, center)
            .is_some_and(|other| distance_squared(center, other).sqrt() <= FLAT_NEIGHBOR_REACH)
    }
}

/// 计价参数，单位是词网格的自然对数分。`sigma_*`、`bias_y` 以键为单位。默认值在三端宿主导出的几何上扫参得到（见模块注释里的笔记）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TouchTuning {
    /// B：落点在两键正中间时的罚分。
    pub base: f64,
    /// s：距离平方之差每一单位加多少罚分。
    pub scale: f64,
    /// σx、σy：横向、纵向的距离各以多少个键为一个单位。
    pub sigma_x: f64,
    pub sigma_y: f64,
    /// b_y：每个键的瞄准点往下挪多少个键高（按键普遍偏下）。
    pub bias_y: f64,
    /// P_MIN、P_MAX：罚分的下限和上限。平面表原有的邻键永不丢弃，最多付到 P_MAX。
    pub min_penalty: f64,
    pub max_penalty: f64,
    /// P_CUT：平面表里没有的键（真实布局下的斜邻键、`;`）罚分超过它就不加。
    pub cut: f64,
    /// 纠错整句的分数（已经扣了罚分）不比字面整句低出这么多时，落点算是支持它：它留在引擎给的座位上（整句块之后），不和整句的次选读法一起挪到表尾。打对了的输入按在键心，罚分高，纠错整句多半低于这条线，照旧沉底，不挤占它后面的候选。
    pub show_margin: f64,
    /// 同时要求纠错整句改动的每一键在落点下的价都不超过它，落点才算支持这次改正。打对的输入里，按在键心附近的那一键改过去的价高，挡在这条线外；只看分数差时，字面整句本来就排得很靠后的那些用例常有一条分数接近的纠错整句，它坐到整句块之后，把金标准往后挤一位。
    pub show_penalty: f64,
}

impl Default for TouchTuning {
    /// 在 Android 模拟器导出的几何（`resources/eval/key-geometry/android-portrait*.json`）上扫参得到；扫参表见模块注释里的笔记。
    fn default() -> Self {
        Self {
            base: 8.0,
            scale: 12.0,
            sigma_x: 1.0,
            sigma_y: 1.0,
            bias_y: 0.0,
            min_penalty: 4.0,
            max_penalty: 28.0,
            cut: 16.0,
            show_margin: 6.0,
            show_penalty: 14.0,
        }
    }
}

/// 有限且不超过 [`COORDINATE_LIMIT`]；NaN 不满足。
fn within_limit(value: f32) -> bool {
    value.abs() <= COORDINATE_LIMIT
}

/// `unit` 和 `keys` 的位模式的哈希。先加 0.0 把 -0.0 归成 0.0，内容按 `==` 相同的几何指纹也相同（NaN 在构造时已被拒绝）。
fn content_fingerprint(unit: (f32, f32), keys: &[(u8, f32, f32)]) -> u64 {
    use std::hash::{Hash, Hasher};
    let bits = |value: f32| (value + 0.0).to_bits();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bits(unit.0).hash(&mut hasher);
    bits(unit.1).hash(&mut hasher);
    for &(key, x, y) in keys {
        key.hash(&mut hasher);
        bits(x).hash(&mut hasher);
        bits(y).hash(&mut hasher);
    }
    hasher.finish()
}

fn distance_squared((ax, ay): (f32, f32), (bx, by): (f32, f32)) -> f32 {
    (ax - bx) * (ax - bx) + (ay - by) * (ay - by)
}

#[cfg(test)]
mod tests;
