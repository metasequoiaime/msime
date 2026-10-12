//! 脚本插件的候选变换（重排、注释、过滤）在 runtime 里的一侧，排在内置排序（`order_presentation`）之后。
//!
//! - **什么时候运行脚本。** 只在派发类调用（[`Runtime::dispatch`]）的 refresh 和展开之后调用 [`CandidateTransform`]。旁路调用（`apply_online_*`、`resolve/clear_online_candidates`、`rerank_settled`、`all_candidates`、`apply_translations` 和设置类调用）不运行任何脚本代码，只按候选身份重放本组字内最近一次记下的操作，免得把打字停顿和网络时序透露给脚本。
//! - **插件看到什么。** 永远是内置排序之后、任何插件变换之前的窗口：前 [`MAX_TRANSFORM_ROWS`] 行，引擎原始的注释。多个插件的操作都针对这份原始窗口，按候选身份记下，再按授权顺序合成：重排是后面的插件优先，隐藏取并集，注释后面的覆盖前面的。
//! - **重排**同步移动全部平行数组和 `engine_order`，选中的永远是显示的那一行。**过滤不删行**：被隐藏的行降到列表末尾，视图、分页、高亮和选择 ID 只认可见前缀；`engine_order` 仍覆盖引擎的全部候选，座位映射不会错位。
//! - **不能动的行。** 引擎下标 0 的行不可隐藏：引擎有自己按首位取词的路径（失焦上屏、14 键原样上屏等），它取的永远是这一行，所以它必须可见，也因此至少留一行可见。锁定的座位既不移动也不隐藏：固定了位置的词、云和 AI（含占位）行、五笔四码唯一时的 0 号座位（第四键自动上屏的就是它）。
//! - **跳过**：平行数组长度不一致、整句改字中、方案的候选列表打开着（韩文汉字）。本地模式的白名单是插件一侧的规则，由宿主的实现判断。
//! - **暂停**（[`Runtime::suspend_candidate_transform`]）：派发里不运行插件，旁路调用也不重放，记下的操作清掉，列表保持内置排序。宿主在上屏只能观察（`COMMIT_OBSERVE_ONLY`）时暂停：Windows 上 TIP 在自己的会话里选词的列表按那个会话的顺序选，这边重排或隐藏，显示的顺序就和数字选中的对不上。
//!
//! 只在桌面编译（macOS、Windows、非 ohos 的 Linux）。移动端和网页引擎里同名的几个方法什么都不做，可见前缀就是整个列表。

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
pub(crate) use desktop::PluginState;
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
pub use desktop::{CandidateOps, CandidateTransform, TransformRow, MAX_TRANSFORM_ROWS};

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
mod desktop {
    use crate::*;
    use msime_engine::ordering::apply_order;
    use std::collections::HashMap;

    /// 交给插件的窗口最多几行。
    pub const MAX_TRANSFORM_ROWS: usize = 20;

    const CLOUD: u8 = 2;
    const AI: u8 = 3;
    const AI_PENDING: u8 = 12;

    /// 窗口里的一行，插件变换之前的样子：内置排序之后的顺序，引擎原始的注释。
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct TransformRow<'a> {
        pub text: &'a str,
        pub code: &'a str,
        pub annotation: &'a str,
        pub source: u8,
    }

    /// 一个插件对窗口给出的操作。下标都是窗口里的行号；越界的下标被忽略。
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct CandidateOps {
        /// 这些行按给出的顺序排到前面（锁定的座位除外），其余行保持原来的相对顺序。
        pub order: Vec<u32>,
        /// 降到末尾并隐藏的行。引擎下标 0 的行和锁定的座位不会被隐藏。
        pub hidden: Vec<u32>,
        /// 行号和注释。注释显示在 `Candidate::script_annotation`，空串表示不加。
        pub annotations: Vec<(u32, String)>,
    }

    /// 宿主实现的候选变换：按授权顺序运行每个插件，把成功的插件各自的操作依次放进 `out`。失败的插件不放，它这次的操作全部作废。
    ///
    /// 只在派发类调用里被调用；实现不能回调 runtime。
    pub trait CandidateTransform {
        fn transform(
            &mut self,
            window: &[TransformRow<'_>],
            local_mode: &str,
            out: &mut Vec<CandidateOps>,
        );
    }

    /// 候选的身份：文本、来源、编码和同组在引擎原始顺序中的序号。
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Identity {
        text: String,
        source: u8,
        code: String,
        occurrence: usize,
    }

    impl Identity {
        fn all(snapshot: &EngineSnapshot, engine_order: &[usize]) -> Vec<Self> {
            let count = snapshot.candidates.len();
            let mut seats: Vec<usize> = (0..count).collect();
            seats.sort_unstable_by_key(|seat| engine_order.get(*seat).copied().unwrap_or(*seat));
            let mut occurrences: HashMap<(&str, u8, &str), usize> = HashMap::new();
            let mut identities = vec![None; count];
            for seat in seats {
                let text = snapshot.candidates[seat].as_str();
                let source = snapshot.candidate_sources[seat];
                let code = snapshot.candidate_codes[seat].as_str();
                let occurrence = occurrences.entry((text, source, code)).or_default();
                identities[seat] = Some(Self {
                    text: text.to_owned(),
                    source,
                    code: code.to_owned(),
                    occurrence: *occurrence,
                });
                *occurrence += 1;
            }
            identities.into_iter().map(Option::unwrap).collect()
        }
    }

    /// 挂在 runtime 上的插件状态。
    #[derive(Default)]
    pub(crate) struct PluginState {
        transform: Option<Box<dyn CandidateTransform>>,
        /// 正在派发类调用里：这时的 refresh 和展开运行脚本，其余时候只重放。
        live: bool,
        /// 各插件这一次交回的操作，缓冲区在调用之间复用。
        ops: Vec<CandidateOps>,
        /// 本组字内最近一次合成的结果，按候选身份。`order` 按优先次序排好。
        order: Vec<Identity>,
        hidden: Vec<Identity>,
        annotations: Vec<(Identity, String)>,
        /// 与当前缓存座位平行；排列后仍能把注释对应回原始候选。
        identities: Vec<Identity>,
        /// 降到末尾、不显示的行数。
        hidden_rows: usize,
        /// 宿主暂停了候选变换：不运行、不重放。
        suspended: bool,
    }

    impl PluginState {
        fn clear_recorded(&mut self) {
            self.order.clear();
            self.hidden.clear();
            self.annotations.clear();
        }

        fn has_recorded(&self) -> bool {
            !self.order.is_empty() || !self.hidden.is_empty() || !self.annotations.is_empty()
        }
    }

    impl<E: InputEngine> Runtime<E> {
        /// 挂上或去掉候选变换。宿主（host-api）在会话选择加入生命周期时挂上脚本插件的实现；去掉时记下的操作一并清掉。
        pub fn set_candidate_transform(&mut self, transform: Option<Box<dyn CandidateTransform>>) {
            if transform.is_none() {
                self.plugins.clear_recorded();
            }
            self.plugins.transform = transform;
        }

        /// 暂停或恢复候选变换。暂停期间派发里的 refresh 和展开不运行插件，旁路调用也不重放，记下的操作在下一次排列时清掉，列表保持内置排序；恢复之后从下一次派发起重新运行。宿主在上屏只能观察时暂停，见模块说明。
        pub fn suspend_candidate_transform(&mut self, suspended: bool) {
            self.plugins.suspended = suspended;
        }

        /// 派发一个动作，期间的 refresh 和展开运行候选变换。panic 也要先把标记放下再继续展开，否则之后的旁路调用会运行脚本。
        pub(crate) fn dispatch_live(&mut self, action: Action) -> Result<Transition, RuntimeError> {
            self.plugins.live = true;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.dispatch_action(action)
            }));
            self.plugins.live = false;
            match result {
                Ok(result) => result,
                Err(panic) => std::panic::resume_unwind(panic),
            }
        }

        /// 可见前缀的长度：视图、分页、高亮和选择 ID 只认这一段。
        pub(crate) fn visible_len(&self) -> usize {
            let count = self.cached.candidates.len();
            count - self.plugins.hidden_rows.min(count)
        }

        /// 本组字内有没有记下的插件操作。
        pub(crate) fn has_candidate_ops(&self) -> bool {
            self.plugins.has_recorded()
        }

        /// 每个座位上的引擎下标，`engine_order` 为空时就是座位本身。
        pub(crate) fn seat_order(&self) -> Vec<usize> {
            let count = self.cached.candidates.len();
            if self.engine_order.len() == count {
                self.engine_order.clone()
            } else {
                (0..count).collect()
            }
        }

        /// 当前列表能不能做插件变换：没有暂停、有候选、平行数组等长、不在整句改字、方案的候选列表没打开。
        fn transform_ready(&self) -> bool {
            let snapshot = &self.cached;
            let count = snapshot.candidates.len();
            !self.plugins.suspended
                && count > 0
                && snapshot.candidate_codes.len() == count
                && snapshot.candidate_annotations.len() == count
                && snapshot.candidate_sources.len() == count
                && snapshot.candidate_positions.len() == count
                && snapshot.candidate_corrected.len() == count
                && snapshot.candidate_answers_key.len() == count
                && snapshot.conversion.is_empty()
                && !snapshot.candidate_list_open
        }

        /// 内置排序之后调用：派发类调用里运行插件、记下它们的操作，其余调用只重放记下的操作。返回隐藏的行数或注释是否变了（重排由调用方比较数组得知）。
        pub(crate) fn transform_candidates(&mut self) -> bool {
            let hidden_before = self.plugins.hidden_rows;
            let ready = self.transform_ready();
            if ready && (self.plugins.transform.is_some() || self.plugins.has_recorded()) {
                self.plugins.identities = Identity::all(&self.cached, &self.engine_order);
            } else {
                self.plugins.identities.clear();
            }
            let mut annotations_changed = false;
            // 暂停时和没有候选时一样把记下的操作清掉：恢复之后的旁路调用不能把暂停之前的操作重放到新列表上。
            if self.cached.candidates.is_empty() || self.plugins.suspended {
                annotations_changed = !self.plugins.annotations.is_empty();
                self.plugins.clear_recorded();
            } else if self.plugins.live {
                let previous = std::mem::take(&mut self.plugins.annotations);
                self.plugins.order.clear();
                self.plugins.hidden.clear();
                if ready {
                    self.run_transform();
                }
                annotations_changed = previous != self.plugins.annotations;
            }
            self.arrange_candidates(ready);
            // 高亮只能落在可见前缀里：派发里的展开会重新运行插件，插件这次可以多藏几行，而翻页填满当前页时高亮原地不动；落进被隐藏的末尾时，空格、结束组字和标点按高亮交给引擎的就是一行被隐藏的候选。
            let visible = self.visible_len();
            if self.highlighted >= visible {
                self.highlighted = visible.saturating_sub(1);
            }
            annotations_changed || self.plugins.hidden_rows != hidden_before
        }

        /// 把窗口交给插件，按身份记下合成后的操作。
        fn run_transform(&mut self) {
            let Some(transform) = self.plugins.transform.as_mut() else {
                return;
            };
            let snapshot = &self.cached;
            let count = snapshot.candidates.len().min(MAX_TRANSFORM_ROWS);
            let mut rows = [TransformRow::default(); MAX_TRANSFORM_ROWS];
            for (seat, row) in rows.iter_mut().enumerate().take(count) {
                *row = TransformRow {
                    text: &snapshot.candidates[seat],
                    code: &snapshot.candidate_codes[seat],
                    annotation: &snapshot.candidate_annotations[seat],
                    source: snapshot.candidate_sources[seat],
                };
            }
            let window = &rows[..count];
            self.plugins.ops.clear();
            transform.transform(window, &snapshot.local_mode, &mut self.plugins.ops);
            let plugins = &mut self.plugins;
            // 重排：后面的插件优先，所以倒着收，同一行只收第一次。
            for ops in plugins.ops.iter().rev() {
                for id in ops
                    .order
                    .iter()
                    .filter_map(|index| plugins.identities.get(*index as usize))
                {
                    if !plugins.order.contains(id) {
                        plugins.order.push(id.clone());
                    }
                }
            }
            for ops in &plugins.ops {
                for id in ops
                    .hidden
                    .iter()
                    .filter_map(|index| plugins.identities.get(*index as usize))
                {
                    if !plugins.hidden.contains(id) {
                        plugins.hidden.push(id.clone());
                    }
                }
                for (index, text) in &ops.annotations {
                    let Some(id) = plugins.identities.get(*index as usize) else {
                        continue;
                    };
                    match plugins
                        .annotations
                        .iter_mut()
                        .find(|(existing, _)| existing == id)
                    {
                        Some((_, annotation)) => annotation.clone_from(text),
                        None => plugins.annotations.push((id.clone(), text.clone())),
                    }
                }
            }
        }

        /// 按记下的操作排好当前列表：可见行里，锁定的座位不动，其余座位先放插件点名的行（按优先次序），再放没点名的行（保持原来的相对顺序）；被隐藏的行按引擎原来的次序排在最后。
        fn arrange_candidates(&mut self, ready: bool) {
            if !ready {
                // 跳过的列表不应沿用之前组字中的插件操作；旁路刷新也必须从干净状态开始。
                self.plugins.clear_recorded();
                self.plugins.hidden_rows = 0;
                return;
            }
            if self.plugins.order.is_empty() && self.plugins.hidden.is_empty() {
                self.plugins.hidden_rows = 0;
                return;
            }
            let snapshot = &self.cached;
            let count = snapshot.candidates.len();
            let engine_index = |seat: usize| self.engine_order.get(seat).copied().unwrap_or(seat);
            let locked = |seat: usize| {
                snapshot.candidate_positions[seat] > 0
                    || matches!(snapshot.candidate_sources[seat], CLOUD | AI | AI_PENDING)
                    || (seat == 0 && snapshot.wubi_unique_four_code)
            };
            let listed = |list: &[Identity], seat: usize| {
                list.iter()
                    .position(|id| id == &self.plugins.identities[seat])
            };
            let hidden: Vec<bool> = (0..count)
                .map(|seat| {
                    !locked(seat)
                        && engine_index(seat) != 0
                        && listed(&self.plugins.hidden, seat).is_some()
                })
                .collect();
            let hidden_rows = hidden.iter().filter(|hidden| **hidden).count();
            let visible = count - hidden_rows;
            let stays = |seat: usize| seat < visible && locked(seat);
            // 不留在原座位上的可见行：点名的（锁定的行不认点名）按优先次序在前，其余按原来的座位。
            let mut free: Vec<(usize, usize)> = (0..count)
                .filter(|seat| !hidden[*seat] && !stays(*seat))
                .map(|seat| {
                    let rank = if locked(seat) {
                        None
                    } else {
                        listed(&self.plugins.order, seat)
                    };
                    (rank.unwrap_or(usize::MAX), seat)
                })
                .collect();
            free.sort_unstable();
            let mut free = free.into_iter().map(|(_, seat)| seat);
            let mut order = Vec::with_capacity(count);
            for seat in 0..visible {
                if stays(seat) {
                    order.push(seat);
                } else if let Some(row) = free.next() {
                    order.push(row);
                }
            }
            // 隐藏的行按引擎原来的次序排：末尾里的位置变动（落定重排提上来一行隐藏行之类）不改变座位顺序，不会被当成可见行挪动过。
            let mut tail: Vec<usize> = (0..count).filter(|seat| hidden[*seat]).collect();
            tail.sort_unstable_by_key(|seat| engine_index(*seat));
            order.extend(tail);
            // 一个排列，否则什么都不动：缺一个或重一个下标都会悄悄丢掉候选。
            debug_assert_eq!(order.len(), count);
            if order.len() != count {
                self.plugins.hidden_rows = 0;
                return;
            }
            self.plugins.hidden_rows = hidden_rows;
            if order.iter().enumerate().all(|(seat, row)| seat == *row) {
                return;
            }
            self.apply_candidate_order(&order);
            apply_order(&mut self.plugins.identities, &order);
        }

        /// `seat` 上可见的行的插件注释；没有、为空或者列表现在不做变换时为 `None`。
        pub(crate) fn script_annotation(&self, seat: usize) -> Option<String> {
            if self.plugins.annotations.is_empty()
                || seat >= self.visible_len()
                || !self.transform_ready()
            {
                return None;
            }
            let identity = self.plugins.identities.get(seat)?;
            self.plugins
                .annotations
                .iter()
                .find(|(id, _)| id == identity)
                .map(|(_, annotation)| annotation)
                .filter(|annotation| !annotation.is_empty())
                .cloned()
        }
    }
}

/// 移动端和网页引擎：没有候选变换，可见前缀就是整个列表。
#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
)))]
mod inert {
    use crate::*;

    impl<E: InputEngine> Runtime<E> {
        pub(crate) fn visible_len(&self) -> usize {
            self.cached.candidates.len()
        }

        pub(crate) fn has_candidate_ops(&self) -> bool {
            false
        }

        pub(crate) fn seat_order(&self) -> Vec<usize> {
            Vec::new()
        }

        pub(crate) fn transform_candidates(&mut self) -> bool {
            false
        }
    }
}
