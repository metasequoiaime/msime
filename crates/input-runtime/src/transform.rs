//! 脚本插件的候选变换（重排、注释、过滤）在 runtime 里的一侧，排在内置排序（`order_presentation`）之后。
//!
//! - **什么时候运行脚本。** 只在派发类调用（[`Runtime::dispatch`]）的 refresh 和展开之后调用 [`CandidateTransform`]。旁路调用（`apply_online_*`、`resolve/clear_online_candidates`、`rerank_settled`、`all_candidates`、`apply_translations` 和设置类调用）不运行任何脚本代码，只按候选身份重放本组字内最近一次记下的操作，免得把打字停顿和网络时序透露给脚本。
//! - **插件看到什么。** 永远是内置排序之后、任何插件变换之前的窗口：前 [`MAX_TRANSFORM_ROWS`] 行，引擎原始的注释。多个插件的操作都针对这份原始窗口，按候选身份记下，再按授权顺序合成：重排是后面的插件优先，隐藏取并集，注释后面的覆盖前面的。
//! - **插入的行**（`insert_candidates`）在重排、隐藏、注释合成之后按执行顺序插进可见行里，位置是插件变换之后的呈现顺序里的位置，所有插件合计最多 [`MAX_INSERTED_ROWS`] 行，同文的只留第一行；锁定的座位不因插入挪位。插入的行来源是 `Plugin`，在列表里没有引擎下标：选中、空格、数字、点选、标点结束时 runtime 自己把它原样上屏并丢掉组字（不经过引擎，所以不进词库、不调词频），不计打字统计、不作为 AI 上文；以词定字、置顶、删除、固定位置对它什么都不做；输入法替失焦结束组字时上屏的是引擎的首行，不是插入的行。只在本地模式 `none`、没有暂存的词段、不是九键或宫格时插入。旁路调用按记下的位置和那里（插在末尾时是最后一行）候选的身份重放，身份对不上的那一行丢弃，不猜位置。有插入的行时不做落定重排：落定重排按座位挪行，会把插入的行当成引擎的候选。
//! - **重排**同步移动全部平行数组和 `engine_order`，选中的永远是显示的那一行。**过滤不删行**：被隐藏的行降到列表末尾，视图、分页、高亮和选择 ID 只认可见前缀；`engine_order` 仍覆盖引擎的全部候选，座位映射不会错位。
//! - **不能动的行。** 引擎下标 0 的行不可隐藏：引擎有自己按首位取词的路径（失焦上屏、14 键原样上屏等），它取的永远是这一行，所以它必须可见，也因此至少留一行可见。锁定的座位既不移动也不隐藏：固定了位置的词、云和 AI（含占位）行、五笔四码唯一时的 0 号座位（第四键自动上屏的就是它）。
//! - **跳过**：平行数组长度不一致、整句改字中、方案的候选列表打开着（韩文汉字）。本地模式的白名单是插件一侧的规则，由宿主的实现判断。
//! - **暂停**（[`Runtime::suspend_candidate_transform`]）：派发里不运行插件，旁路调用也不重放，记下的操作清掉，列表保持内置排序。宿主在上屏只能观察（`COMMIT_OBSERVE_ONLY`）时暂停：Windows 上 TIP 在自己的会话里选词的列表按那个会话的顺序选，这边重排或隐藏，显示的顺序就和数字选中的对不上。
//! - **插件指令**（[`CommandProvider`]）：`/` 模式里输入以某个插件触发词开头时，由触发词的主人给出行（来源 14）。只在派发类调用的 refresh 开头、取快照之前、没有暂停、引擎有要问的查询（[`InputEngine::plugin_command_query`]）时调用；行在同一次派发里写进引擎，代次不变。引擎按预编辑串缓存这些行，所以旁路调用、方向键和翻页都不再调用插件。行的文字只画在插件注释格（「标题 注释」），引擎注释格留空。
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
pub use desktop::{
    CandidateOps, CandidateTransform, CommandOutcome, CommandProvider, InsertedRow, TransformRow,
    MAX_ANNOTATION_UTF16, MAX_INSERTED_ROWS, MAX_TRANSFORM_ROWS,
};

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
mod desktop {
    use crate::*;
    use msime_engine::ordering::apply_order;
    use std::collections::HashMap;
    use std::hash::{Hash, Hasher};

    /// 交给插件的窗口最多几行。
    pub const MAX_TRANSFORM_ROWS: usize = 20;

    /// 插件注释格最多几个 UTF-16 码元，和 script-runtime 的 `limits::MAX_ANNOTATION_UTF16` 同值。插件指令行的「标题 注释」按它截断。
    pub const MAX_ANNOTATION_UTF16: usize = 32;

    /// 所有插件合计每次最多插几行，和 script-runtime 的 `limits::MAX_INSERTED_ROWS` 同值。
    pub const MAX_INSERTED_ROWS: usize = 6;

    /// 插入的行最多几个 UTF-16 码元，和插件指令行同一个上限（script-runtime 的 `limits::MAX_COMMAND_ROW_UTF16`）。
    const MAX_INSERTED_ROW_UTF16: usize = 64;

    /// 插入的行在 `engine_order` 里的下标：它不是引擎的候选。
    pub(crate) const INSERTED: usize = usize::MAX;

    const CLOUD: u8 = 2;
    const CLOUD_PENDING: u8 = 15;
    const AI: u8 = 3;
    const AI_PENDING: u8 = 12;
    const PLUGIN: u8 = msime_engine::CandidateSource::Plugin as u8;

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
        /// 要插进列表的行，按暂存的顺序。
        pub inserts: Vec<InsertedRow>,
    }

    /// 一个插件要插进候选列表的一行。宿主已经按 `command_row` 的规则校验过文本和注释；runtime 仍然丢掉它用不了的（空白、含控制字符、太长）。
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct InsertedRow {
        /// 插件变换之后的呈现顺序里的位置，0 到窗口行数；超出可见行数时插在可见行的末尾。
        pub position: u32,
        pub text: String,
        /// 显示在插件注释格，空串表示不加。
        pub note: String,
    }

    /// 记下的一次插入：插到哪里、插什么，以及插入时那个位置上候选的身份（插在末尾时是最后一行，`after` 为真）。旁路调用按它重放。
    #[derive(Clone, Debug)]
    struct RecordedInsert {
        position: usize,
        text: String,
        note: String,
        anchor: Identity,
        after: bool,
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

    /// 插件指令这一次有没有运行。
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum CommandOutcome {
        /// 插件运行完了，`out` 就是它的行（可以是 0 行）。引擎把结果缓存起来，同一份预编辑串不再问。
        Ran,
        /// 插件没有运行：模块还在编译、已经熔断、本轮配额用完、没有这个触发词的主人。不写缓存，下一次派发再问。
        NotRun,
    }

    /// 宿主实现的插件指令：把触发词和参数交给拥有这个触发词的那一个插件，把它给的行（文本，插件注释）放进 `out`。
    ///
    /// 只在派发类调用里被调用；实现不能回调 runtime。
    pub trait CommandProvider {
        fn provide(
            &mut self,
            trigger: &str,
            argument: &str,
            out: &mut Vec<(String, String)>,
        ) -> CommandOutcome;
    }

    /// 候选的身份：文本、来源、编码和同组在引擎原始顺序中的序号。
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Identity {
        text: String,
        source: u8,
        code: String,
        occurrence: usize,
    }

    #[derive(Default)]
    struct ArrangementBuffers {
        hidden: Vec<bool>,
        free: Vec<(usize, usize)>,
        order: Vec<usize>,
        tail: Vec<usize>,
    }

    #[derive(Default)]
    struct OccurrenceSlot {
        hash: u64,
        representative: usize,
        count: usize,
        generation: u64,
    }

    #[derive(Default)]
    struct OccurrenceBuffers {
        seats: Vec<usize>,
        slots: Vec<OccurrenceSlot>,
        generation: u64,
    }

    impl Identity {
        fn fill_occurrences(
            snapshot: &EngineSnapshot,
            engine_order: &[usize],
            buffers: &mut OccurrenceBuffers,
            occurrences: &mut Vec<usize>,
        ) {
            let count = snapshot.candidates.len();
            buffers.seats.clear();
            buffers.seats.extend(0..count);
            buffers
                .seats
                .sort_unstable_by_key(|seat| engine_order.get(*seat).copied().unwrap_or(*seat));
            occurrences.clear();
            occurrences.resize(count, 0);

            let required_slots = count.saturating_mul(2).max(1);
            if buffers.slots.len() < required_slots {
                buffers
                    .slots
                    .resize_with(required_slots.next_power_of_two(), Default::default);
            }
            let next_generation = buffers.generation.wrapping_add(1);
            if next_generation == 0 {
                for slot in &mut buffers.slots {
                    slot.generation = 0;
                }
                buffers.generation = 1;
            } else {
                buffers.generation = next_generation;
            }
            let generation = buffers.generation;
            for &seat in &buffers.seats {
                let text = snapshot.candidates[seat].as_str();
                let source = snapshot.candidate_sources[seat];
                let code = snapshot.candidate_codes[seat].as_str();
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                text.hash(&mut hasher);
                source.hash(&mut hasher);
                code.hash(&mut hasher);
                let hash = hasher.finish();
                let mut slot = (hash as usize) & (buffers.slots.len() - 1);
                loop {
                    let entry = &mut buffers.slots[slot];
                    if entry.generation != generation {
                        *entry = OccurrenceSlot {
                            hash,
                            representative: seat,
                            count: 1,
                            generation,
                        };
                        break;
                    }
                    if entry.hash == hash
                        && snapshot.candidates[entry.representative] == snapshot.candidates[seat]
                        && snapshot.candidate_sources[entry.representative]
                            == snapshot.candidate_sources[seat]
                        && snapshot.candidate_codes[entry.representative]
                            == snapshot.candidate_codes[seat]
                    {
                        occurrences[seat] = entry.count;
                        entry.count += 1;
                        break;
                    }
                    slot = (slot + 1) & (buffers.slots.len() - 1);
                }
            }
        }

        fn at(snapshot: &EngineSnapshot, seat: usize, occurrence: usize) -> Self {
            Self {
                text: snapshot.candidates[seat].clone(),
                source: snapshot.candidate_sources[seat],
                code: snapshot.candidate_codes[seat].clone(),
                occurrence,
            }
        }

        fn matches(&self, snapshot: &EngineSnapshot, seat: usize, occurrence: usize) -> bool {
            self.occurrence == occurrence
                && self.source == snapshot.candidate_sources[seat]
                && self.text == snapshot.candidates[seat]
                && self.code == snapshot.candidate_codes[seat]
        }
    }

    fn annotation_text(title: &str, note: &str) -> Option<String> {
        if title.is_empty() && note.is_empty() {
            return None;
        }
        let separator = !title.is_empty() && !note.is_empty();
        // 一个 UTF-16 码元最多对应三个 UTF-8 字节，32 码元的结果不会超过此容量。
        let capacity = title
            .len()
            .saturating_add(note.len())
            .saturating_add(usize::from(separator))
            .min(MAX_ANNOTATION_UTF16 * 3);
        let mut text = String::with_capacity(capacity);
        let mut units = 0;
        for character in title
            .chars()
            .chain(separator.then_some(' '))
            .chain(note.chars())
        {
            units += character.len_utf16();
            if units > MAX_ANNOTATION_UTF16 {
                break;
            }
            text.push(character);
        }
        Some(text)
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
        /// 与当前缓存座位平行的同组出现序号；文本和编码借用快照，仅被点名的行保存完整身份。
        occurrences: Vec<usize>,
        /// 构造同组出现序号时复用的座位排序和开放寻址表。
        occurrence_buffers: OccurrenceBuffers,
        /// 降到末尾、不显示的行数。
        hidden_rows: usize,
        /// 这一次派发里插件交回的插入，按执行顺序，等排列之后再定锚点。
        pending_inserts: Vec<InsertedRow>,
        /// 本组字内最近一次派发记下的插入，按执行顺序。
        inserts: Vec<RecordedInsert>,
        /// 当前列表里插入的行：（座位，在 `inserts` 里的下标），座位升序。
        inserted: Vec<(usize, usize)>,
        /// 宿主暂停了候选变换：不运行、不重放。插件指令同样不调用。
        suspended: bool,
        /// 插件指令的提供者，见 [`CommandProvider`]。
        commands: Option<Box<dyn CommandProvider>>,
        /// 最近一次写进引擎的插件行的注释，按行文本；同文的行引擎只留第一行，这里也只记第一条。
        command_notes: HashMap<String, String>,
        /// 插件指令提供者的回包行，调用之间清空并复用容量。
        command_rows: Vec<(String, String)>,
        /// 候选排列的工作区，旁路重放之间清空并复用容量。
        arrangement: ArrangementBuffers,
    }

    impl PluginState {
        fn clear_recorded(&mut self) {
            self.order.clear();
            self.hidden.clear();
            self.annotations.clear();
            self.inserts.clear();
        }

        fn has_recorded(&self) -> bool {
            !self.order.is_empty()
                || !self.hidden.is_empty()
                || !self.annotations.is_empty()
                || !self.inserts.is_empty()
        }

        fn inserted_index(&self, seat: usize) -> Option<usize> {
            self.inserted
                .binary_search_by_key(&seat, |(inserted, _)| *inserted)
                .ok()
                .map(|position| self.inserted[position].1)
        }
    }

    /// 插入的行能不能用：不空白、首尾不是空白、不含控制字符、不超过 [`MAX_INSERTED_ROW_UTF16`] 个码元；注释不含控制字符、不超过 [`MAX_ANNOTATION_UTF16`] 个码元。宿主已经校验得更严，这里只是下限。
    fn usable_insert(row: &InsertedRow) -> bool {
        !row.text.trim().is_empty()
            && row.text.trim() == row.text
            && !row.text.chars().any(char::is_control)
            && row.text.encode_utf16().count() <= MAX_INSERTED_ROW_UTF16
            && !row.note.chars().any(char::is_control)
            && row.note.encode_utf16().count() <= MAX_ANNOTATION_UTF16
    }

    /// 排好之后的列表里的一个位置：原来的座位，或者 `inserts` 里的第几行。
    #[derive(Clone, Copy)]
    enum Slot {
        Seat(usize),
        Insert(usize),
    }

    impl<E: InputEngine> Runtime<E> {
        /// 挂上或去掉候选变换。宿主（host-api）在会话选择加入生命周期时挂上脚本插件的实现；去掉时记下的操作一并清掉。
        pub fn set_candidate_transform(&mut self, transform: Option<Box<dyn CandidateTransform>>) {
            if transform.is_none() {
                self.plugins.clear_recorded();
            }
            self.plugins.transform = transform;
        }

        /// 挂上或去掉插件指令的提供者。宿主（host-api）在挂候选变换的同一处挂上；去掉时记下的注释一并清掉。
        pub fn set_command_provider(&mut self, provider: Option<Box<dyn CommandProvider>>) {
            if provider.is_none() {
                self.plugins.command_notes.clear();
            }
            self.plugins.commands = provider;
        }

        /// refresh 开头、取快照之前调用：派发类调用里、没有暂停、挂着提供者、引擎有要问的查询时问插件，插件运行了就把行写进引擎。旁路调用一概不问，引擎缓存的行照样显示。
        pub(crate) fn run_command_provider(&mut self) {
            if !self.plugins.live || self.plugins.suspended {
                return;
            }
            let Some(provider) = self.plugins.commands.as_mut() else {
                return;
            };
            let Some(query) = self.engine.plugin_command_query() else {
                return;
            };
            self.plugins.command_rows.clear();
            let outcome = provider.provide(
                &query.trigger,
                &query.argument,
                &mut self.plugins.command_rows,
            );
            if outcome == CommandOutcome::NotRun {
                self.plugins.command_rows.clear();
                return;
            }
            if self
                .engine
                .apply_plugin_command_rows(&query, &self.plugins.command_rows)
            {
                let notes = &mut self.plugins.command_notes;
                notes.clear();
                for (text, note) in self.plugins.command_rows.drain(..) {
                    notes.entry(text).or_insert(note);
                }
            } else {
                self.plugins.command_rows.clear();
            }
        }

        /// 插件指令行的插件注释：「标题 注释」，没有注释时只有标题，截到 [`MAX_ANNOTATION_UTF16`] 个码元；含控制字符的注释不显示。标题向引擎要（`InputEngine::plugin_command_title`），不在这里另记一份：宿主交来的表里触发词重复或标题不合规时，只有引擎知道哪一条生效。
        fn plugin_row_annotation(&self, seat: usize) -> Option<String> {
            let text = self.cached.candidates.get(seat)?;
            let trigger = self.cached.candidate_codes.get(seat)?;
            let title = self
                .engine
                .plugin_command_title(trigger)
                .unwrap_or_default();
            let note = self
                .plugins
                .command_notes
                .get(text)
                .map(String::as_str)
                .filter(|note| !note.chars().any(char::is_control))
                .unwrap_or_default();
            annotation_text(title, note)
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
        #[cfg(test)]
        pub(crate) fn seat_order(&self) -> Vec<usize> {
            let mut order = Vec::new();
            self.copy_seat_order(&mut order);
            order
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
                && !snapshot.caret_edit_suffix
                && !snapshot.candidate_list_open
        }

        /// 内置排序之后调用：派发类调用里运行插件、记下它们的操作，其余调用只重放记下的操作。返回隐藏的行数或注释是否变了（重排由调用方比较数组得知）。
        pub(crate) fn transform_candidates(&mut self) -> bool {
            // 列表里还留着上一次插入的行（落定整理在原列表上重新排列时）：先拿掉，排完再按记下的插入放回。
            let inserted_before = self.remove_inserted_rows();
            let hidden_before = self.plugins.hidden_rows;
            let ready = self.transform_ready();
            let live = self.plugins.live;
            let mut annotations_changed = false;
            // 暂停时和没有候选时一样把记下的操作清掉：恢复之后的旁路调用不能把暂停之前的操作重放到新列表上。
            if self.cached.candidates.is_empty() || self.plugins.suspended {
                self.plugins.occurrences.clear();
                annotations_changed = !self.plugins.annotations.is_empty();
                self.plugins.clear_recorded();
            } else if self.plugins.live {
                self.plugins.occurrences.clear();
                let previous = std::mem::take(&mut self.plugins.annotations);
                self.plugins.order.clear();
                self.plugins.hidden.clear();
                if ready {
                    self.run_transform();
                }
                annotations_changed = previous != self.plugins.annotations;
            } else if ready && self.plugins.has_recorded() {
                Identity::fill_occurrences(
                    &self.cached,
                    &self.engine_order,
                    &mut self.plugins.occurrence_buffers,
                    &mut self.plugins.occurrences,
                );
            } else {
                self.plugins.occurrences.clear();
            }
            self.arrange_candidates(ready);
            let inserted_changed = self.insert_rows(ready, live) || inserted_before;
            // 高亮只能落在可见前缀里：派发里的展开会重新运行插件，插件这次可以多藏几行，而翻页填满当前页时高亮原地不动；落进被隐藏的末尾时，空格、结束组字和标点按高亮交给引擎的就是一行被隐藏的候选。
            let visible = self.visible_len();
            if self.highlighted >= visible {
                self.highlighted = visible.saturating_sub(1);
            }
            annotations_changed || self.plugins.hidden_rows != hidden_before || inserted_changed
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
            plugins.pending_inserts.clear();
            for ops in &mut plugins.ops {
                plugins.pending_inserts.append(&mut ops.inserts);
            }
            // 没有操作时不建立全列表元数据；插件仍收到原来的窗口并照常运行。
            if !plugins.ops.iter().any(|ops| {
                !ops.order.is_empty() || !ops.hidden.is_empty() || !ops.annotations.is_empty()
            }) {
                return;
            }
            Identity::fill_occurrences(
                snapshot,
                &self.engine_order,
                &mut plugins.occurrence_buffers,
                &mut plugins.occurrences,
            );
            // 重排：后面的插件优先，所以倒着收，同一行只收第一次。
            for ops in plugins.ops.iter().rev() {
                for &index in &ops.order {
                    let seat = index as usize;
                    let Some(&occurrence) = plugins.occurrences.get(seat) else {
                        continue;
                    };
                    if !plugins
                        .order
                        .iter()
                        .any(|id| id.matches(snapshot, seat, occurrence))
                    {
                        plugins.order.push(Identity::at(snapshot, seat, occurrence));
                    }
                }
            }
            for ops in &plugins.ops {
                for &index in &ops.hidden {
                    let seat = index as usize;
                    let Some(&occurrence) = plugins.occurrences.get(seat) else {
                        continue;
                    };
                    if !plugins
                        .hidden
                        .iter()
                        .any(|id| id.matches(snapshot, seat, occurrence))
                    {
                        plugins
                            .hidden
                            .push(Identity::at(snapshot, seat, occurrence));
                    }
                }
                for (index, text) in &ops.annotations {
                    let seat = *index as usize;
                    let Some(&occurrence) = plugins.occurrences.get(seat) else {
                        continue;
                    };
                    match plugins
                        .annotations
                        .iter_mut()
                        .find(|(id, _)| id.matches(snapshot, seat, occurrence))
                    {
                        Some((_, annotation)) => annotation.clone_from(text),
                        None => plugins
                            .annotations
                            .push((Identity::at(snapshot, seat, occurrence), text.clone())),
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
                    || matches!(
                        snapshot.candidate_sources[seat],
                        CLOUD | CLOUD_PENDING | AI | AI_PENDING
                    )
                    || (seat == 0 && snapshot.wubi_unique_four_code)
            };
            let listed = |list: &[Identity], seat: usize| {
                list.iter()
                    .position(|id| id.matches(snapshot, seat, self.plugins.occurrences[seat]))
            };
            let mut buffers = std::mem::take(&mut self.plugins.arrangement);
            buffers.hidden.clear();
            buffers.hidden.extend((0..count).map(|seat| {
                !locked(seat)
                    && engine_index(seat) != 0
                    && listed(&self.plugins.hidden, seat).is_some()
            }));
            let hidden_rows = buffers.hidden.iter().filter(|hidden| **hidden).count();
            let visible = count - hidden_rows;
            let stays = |seat: usize| seat < visible && locked(seat);
            // 不留在原座位上的可见行：点名的（锁定的行不认点名）按优先次序在前，其余按原来的座位。
            buffers.free.clear();
            buffers.free.extend(
                (0..count)
                    .filter(|seat| !buffers.hidden[*seat] && !stays(*seat))
                    .map(|seat| {
                        let rank = if locked(seat) {
                            None
                        } else {
                            listed(&self.plugins.order, seat)
                        };
                        (rank.unwrap_or(usize::MAX), seat)
                    }),
            );
            buffers.free.sort_unstable();
            let mut free = buffers.free.iter().map(|(_, seat)| *seat);
            buffers.order.clear();
            buffers.order.reserve(count);
            for seat in 0..visible {
                if stays(seat) {
                    buffers.order.push(seat);
                } else if let Some(row) = free.next() {
                    buffers.order.push(row);
                }
            }
            // 隐藏的行按引擎原来的次序排：末尾里的位置变动（落定重排提上来一行隐藏行之类）不改变座位顺序，不会被当成可见行挪动过。
            buffers.tail.clear();
            buffers
                .tail
                .extend((0..count).filter(|seat| buffers.hidden[*seat]));
            buffers
                .tail
                .sort_unstable_by_key(|seat| engine_index(*seat));
            buffers.order.extend(buffers.tail.iter().copied());
            // 一个排列，否则什么都不动：缺一个或重一个下标都会悄悄丢掉候选。
            debug_assert_eq!(buffers.order.len(), count);
            if buffers.order.len() != count {
                self.plugins.hidden_rows = 0;
                self.plugins.arrangement = buffers;
                return;
            }
            self.plugins.hidden_rows = hidden_rows;
            if buffers
                .order
                .iter()
                .enumerate()
                .all(|(seat, row)| seat == *row)
            {
                self.plugins.arrangement = buffers;
                return;
            }
            self.apply_candidate_order(&buffers.order);
            apply_order(&mut self.plugins.occurrences, &buffers.order);
            self.plugins.arrangement = buffers;
        }

        /// 这一次能不能插行：列表能变换，本地模式 `none`，没有暂存的词段，不是九键、宫格或联想。
        fn inserts_allowed(&self) -> bool {
            self.cached.local_mode == "none"
                && self.phrase_prefix.is_empty()
                && !self.cached.nine_key
                && self.cached.key_grid.is_none()
                && !self.cached.prediction
        }

        /// 排列之后把插入的行放进可见行里。派发类调用里用插件这一次交回的插入并记下锚点；旁路调用按记下的插入重放，锚点对不上的丢弃。返回列表里有没有插入的行。
        fn insert_rows(&mut self, ready: bool, live: bool) -> bool {
            let pending = std::mem::take(&mut self.plugins.pending_inserts);
            if !ready || !self.inserts_allowed() {
                self.plugins.inserts.clear();
                return false;
            }
            let count = self.cached.candidates.len();
            let visible = self.visible_len();
            if visible == 0 {
                self.plugins.inserts.clear();
                return false;
            }
            if live && pending.is_empty() && self.plugins.inserts.is_empty() {
                return false;
            }
            if !live && self.plugins.inserts.is_empty() {
                return false;
            }
            if self.plugins.occurrences.len() != count {
                Identity::fill_occurrences(
                    &self.cached,
                    &self.engine_order,
                    &mut self.plugins.occurrence_buffers,
                    &mut self.plugins.occurrences,
                );
            }
            let anchor_of = |runtime: &Self, position: usize| {
                let (seat, after) = if position < visible {
                    (position, false)
                } else {
                    (visible - 1, true)
                };
                (
                    Identity::at(&runtime.cached, seat, runtime.plugins.occurrences[seat]),
                    after,
                )
            };
            if live {
                self.plugins.inserts.clear();
                for row in pending {
                    if self.plugins.inserts.len() == MAX_INSERTED_ROWS {
                        break;
                    }
                    if !usable_insert(&row)
                        || self
                            .plugins
                            .inserts
                            .iter()
                            .any(|recorded| recorded.text == row.text)
                    {
                        continue;
                    }
                    let position = (row.position as usize).min(visible);
                    let (anchor, after) = anchor_of(self, position);
                    self.plugins.inserts.push(RecordedInsert {
                        position,
                        text: row.text,
                        note: row.note,
                        anchor,
                        after,
                    });
                }
            }
            // 锚点对得上的才插：位置钳到现在的可见行数，那里（或末尾的最后一行）的候选还得是记下的那一个。
            let mut accepted: Vec<usize> = Vec::new();
            for (index, recorded) in self.plugins.inserts.iter().enumerate() {
                let position = recorded.position.min(visible);
                let after = position == visible;
                let seat = if after { visible - 1 } else { position };
                if after == recorded.after
                    && recorded
                        .anchor
                        .matches(&self.cached, seat, self.plugins.occurrences[seat])
                {
                    accepted.push(index);
                }
            }
            if accepted.is_empty() {
                return false;
            }
            self.splice_inserts(&accepted, visible);
            true
        }

        /// 把 `accepted`（`inserts` 的下标，执行顺序）插进前 `visible` 行，锁定的座位留在原位，被隐藏的行仍在末尾。
        fn splice_inserts(&mut self, accepted: &[usize], visible: usize) {
            let count = self.cached.candidates.len();
            let snapshot = &self.cached;
            let locked = |seat: usize| {
                snapshot.candidate_positions[seat] > 0
                    || matches!(
                        snapshot.candidate_sources[seat],
                        CLOUD | CLOUD_PENDING | AI | AI_PENDING
                    )
                    || (seat == 0 && snapshot.wubi_unique_four_code)
            };
            let inserts = &self.plugins.inserts;
            let at = |position: usize| {
                accepted
                    .iter()
                    .copied()
                    .filter(move |index| inserts[*index].position.min(visible) == position)
            };
            let mut merged: Vec<Slot> = Vec::with_capacity(visible + accepted.len());
            for seat in 0..visible {
                merged.extend(at(seat).map(Slot::Insert));
                merged.push(Slot::Seat(seat));
            }
            merged.extend(at(visible).map(Slot::Insert));
            let mut free = merged
                .iter()
                .copied()
                .filter(|slot| !matches!(slot, Slot::Seat(seat) if locked(*seat)));
            let total = visible + accepted.len();
            let mut order: Vec<Slot> = Vec::with_capacity(count + accepted.len());
            for slot in 0..total {
                if slot < visible && locked(slot) {
                    order.push(Slot::Seat(slot));
                } else if let Some(next) = free.next() {
                    order.push(next);
                }
            }
            debug_assert_eq!(order.len(), total);
            if order.len() != total {
                return;
            }
            order.extend((visible..count).map(Slot::Seat));
            msime_engine::ordering::ensure_engine_order(&mut self.engine_order, count);
            let snapshot = &mut self.cached;
            let evidence = snapshot.candidate_static_evidence.len() == count;
            let contests = snapshot.candidate_contest.len() == count;
            let touch_supported = snapshot.candidate_touch_supported.len() == count;
            let confident = snapshot.candidate_confident_correction.len() == count;
            let pick = |slot: &Slot| match *slot {
                Slot::Seat(seat) => Some(seat),
                Slot::Insert(_) => None,
            };
            macro_rules! rebuild {
                ($field:expr, $inserted:expr) => {{
                    let old = std::mem::take(&mut $field);
                    let mut old: Vec<Option<_>> = old.into_iter().map(Some).collect();
                    $field = order
                        .iter()
                        .map(|slot| match pick(slot) {
                            Some(seat) => old[seat].take().expect("each seat placed once"),
                            None => ($inserted)(slot),
                        })
                        .collect();
                }};
            }
            let inserts = &self.plugins.inserts;
            let text = |slot: &Slot| match *slot {
                Slot::Insert(index) => inserts[index].text.clone(),
                Slot::Seat(_) => String::new(),
            };
            rebuild!(snapshot.candidates, text);
            rebuild!(snapshot.candidate_codes, |_: &Slot| String::new());
            rebuild!(snapshot.candidate_annotations, |_: &Slot| String::new());
            rebuild!(snapshot.candidate_sources, |_: &Slot| PLUGIN);
            rebuild!(snapshot.candidate_positions, |_: &Slot| 0);
            rebuild!(snapshot.candidate_corrected, |_: &Slot| false);
            rebuild!(snapshot.candidate_answers_key, |_: &Slot| false);
            if evidence {
                rebuild!(snapshot.candidate_static_evidence, |_: &Slot| {
                    Default::default()
                });
            }
            if contests {
                rebuild!(snapshot.candidate_contest, |_: &Slot| {
                    msime_engine::ordering::CONTEST_NONE
                });
            }
            if touch_supported {
                rebuild!(snapshot.candidate_touch_supported, |_: &Slot| false);
            }
            if confident {
                rebuild!(snapshot.candidate_confident_correction, |_: &Slot| false);
            }
            rebuild!(self.engine_order, |_: &Slot| INSERTED);
            rebuild!(self.plugins.occurrences, |_: &Slot| 0);
            self.plugins.inserted = order
                .iter()
                .enumerate()
                .filter_map(|(seat, slot)| match *slot {
                    Slot::Insert(index) => Some((seat, index)),
                    Slot::Seat(_) => None,
                })
                .collect();
        }

        /// 拿掉列表里插入的行，返回之前有没有。高亮跟着它所在的候选。
        pub(crate) fn remove_inserted_rows(&mut self) -> bool {
            if self.plugins.inserted.is_empty() {
                return false;
            }
            let inserted = std::mem::take(&mut self.plugins.inserted);
            let count = self.cached.candidates.len();
            let keep: Vec<bool> = (0..count)
                .map(|seat| {
                    inserted
                        .binary_search_by_key(&seat, |(inserted, _)| *inserted)
                        .is_err()
                })
                .collect();
            let removed_before_highlight = inserted
                .iter()
                .filter(|(seat, _)| *seat < self.highlighted)
                .count();
            fn retain_seats<T>(values: &mut Vec<T>, keep: &[bool]) {
                if values.len() != keep.len() {
                    return;
                }
                let mut seat = 0;
                values.retain(|_| {
                    let kept = keep[seat];
                    seat += 1;
                    kept
                });
            }
            let snapshot = &mut self.cached;
            retain_seats(&mut snapshot.candidates, &keep);
            retain_seats(&mut snapshot.candidate_codes, &keep);
            retain_seats(&mut snapshot.candidate_annotations, &keep);
            retain_seats(&mut snapshot.candidate_sources, &keep);
            retain_seats(&mut snapshot.candidate_positions, &keep);
            retain_seats(&mut snapshot.candidate_corrected, &keep);
            retain_seats(&mut snapshot.candidate_answers_key, &keep);
            retain_seats(&mut snapshot.candidate_static_evidence, &keep);
            retain_seats(&mut snapshot.candidate_contest, &keep);
            retain_seats(&mut snapshot.candidate_touch_supported, &keep);
            retain_seats(&mut snapshot.candidate_confident_correction, &keep);
            retain_seats(&mut self.engine_order, &keep);
            retain_seats(&mut self.plugins.occurrences, &keep);
            self.highlighted = self.highlighted.saturating_sub(removed_before_highlight);
            true
        }

        /// 列表整个换成了引擎的新快照：里面没有插入的行，记下的座位作废（记下的插入留着，供重放）。
        pub(crate) fn forget_inserted_rows(&mut self) {
            self.plugins.inserted.clear();
        }

        /// `seat` 上是插件插入的行。
        pub(crate) fn is_inserted(&self, seat: usize) -> bool {
            self.plugins.inserted_index(seat).is_some()
        }

        /// 列表里有没有插入的行。
        pub(crate) fn has_inserted_rows(&self) -> bool {
            !self.plugins.inserted.is_empty()
        }

        /// 引擎的第一个候选现在的座位：插入的行不算。
        pub(crate) fn first_engine_seat(&self) -> usize {
            (0..self.cached.candidates.len())
                .find(|seat| !self.is_inserted(*seat))
                .unwrap_or(0)
        }

        /// `seat` 上可见的行的插件注释；没有、为空或者列表现在不做变换时为 `None`。插件指令行（来源 14）的注释是「标题 注释」，不经过候选变换；插入的行的注释是插件给的那一个。
        pub(crate) fn script_annotation(&self, seat: usize) -> Option<String> {
            if let Some(index) = self.plugins.inserted_index(seat) {
                return Some(self.plugins.inserts[index].note.clone())
                    .filter(|note| !note.is_empty());
            }
            if self.cached.candidate_sources.get(seat) == Some(&PLUGIN) {
                if seat >= self.visible_len() {
                    return None;
                }
                return self.plugin_row_annotation(seat);
            }
            if self.plugins.annotations.is_empty()
                || seat >= self.visible_len()
                || !self.transform_ready()
            {
                return None;
            }
            let occurrence = *self.plugins.occurrences.get(seat)?;
            self.plugins
                .annotations
                .iter()
                .find(|(id, _)| id.matches(&self.cached, seat, occurrence))
                .map(|(_, annotation)| annotation)
                .filter(|annotation| !annotation.is_empty())
                .cloned()
        }
    }
    #[cfg(test)]
    mod annotation_tests {
        use super::*;
        use crate::tests::allocations;

        // 冻结原来的完整拼接和字符收集，独立对照空值、分隔空格及 UTF-16 边界。
        fn frozen_annotation(title: &str, note: &str) -> Option<String> {
            let joined = match (title.is_empty(), note.is_empty()) {
                (true, true) => return None,
                (false, true) => title.to_owned(),
                (true, false) => note.to_owned(),
                (false, false) => format!("{title} {note}"),
            };
            let mut units = 0;
            Some(
                joined
                    .chars()
                    .take_while(|character| {
                        units += character.len_utf16();
                        units <= MAX_ANNOTATION_UTF16
                    })
                    .collect(),
            )
        }

        #[test]
        fn direct_annotation_preserves_the_frozen_text_and_uses_one_buffer() {
            let values = [
                "".to_owned(),
                "UUID".to_owned(),
                "合成".to_owned(),
                "😀".repeat(8),
                "a".repeat(31),
                "注".repeat(32),
                "😀".repeat(17),
                "混😀合".repeat(12),
            ];
            for title in &values {
                for note in &values {
                    let (expected, old_allocations, old_bytes) =
                        allocations::count_bytes(|| frozen_annotation(title, note));
                    let (actual, new_allocations, new_bytes) =
                        allocations::count_bytes(|| annotation_text(title, note));
                    assert_eq!(actual, expected, "title={title:?}, note={note:?}");
                    assert_eq!(new_allocations, usize::from(actual.is_some()));
                    assert!(new_bytes <= MAX_ANNOTATION_UTF16 * 3);
                    if actual.is_some() {
                        assert!(new_allocations < old_allocations);
                        assert!(new_bytes < old_bytes);
                    }
                }
            }
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

        #[cfg(test)]
        pub(crate) fn seat_order(&self) -> Vec<usize> {
            Vec::new()
        }

        pub(crate) fn transform_candidates(&mut self) -> bool {
            false
        }

        pub(crate) fn run_command_provider(&mut self) {}

        pub(crate) fn forget_inserted_rows(&mut self) {}

        pub(crate) fn is_inserted(&self, _seat: usize) -> bool {
            false
        }

        pub(crate) fn has_inserted_rows(&self) -> bool {
            false
        }

        pub(crate) fn first_engine_seat(&self) -> usize {
            0
        }
    }
}
