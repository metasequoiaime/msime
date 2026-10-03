//! 网页内置输入法的宿主编排：按键路由、候选排序、翻页和上屏上文。
//!
//! 这一层是纯 Rust，不碰浏览器，原生测试可以直接驱动它（`tests/routing.rs`、`tests/parity.rs`）。它做的事和桌面宿主里的 `msime_input_runtime::Runtime` 相同——每次选择都经过 `engine_order` 座位映射，排序决策调用 `msime_engine::ordering` 里同一组函数——只是去掉了网页用不到的部分（在线候选、九键、本地模式、词组暂存），并多了网页自己要管的东西：英文模式开关、引号状态和慢帧熔断。
//!
//! 引擎用的是核心 `msime_engine::Session`，不是 `msime_engine::host::Session`：后者构造时要写翻译 sidecar，在 wasm 上没有文件系统可写。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use msime_engine::ordering::{
    rerank_pick, rotate_to_front, runner_up_order, OrderRow, Reranker, SentenceModel,
};
use msime_engine::time::Instant;
use msime_engine::{
    Command, EnglishInputOptions, FrequencyAdjustmentOptions, FuzzyPinyinOptions, KeyResult,
    LocalInputMode, LocalModeOptions, MixedExpressiveOptions, RuntimePaths, SchemeType,
    SentenceAssociationOptions, Session, SessionOptions, SessionSnapshot, ShuangpinProfileKind,
    WubiInputOptions, WubiProfileKind,
};

/// 上文最多保留的字节数，和 `Runtime::remember_commit` 一样：引擎的查询拒绝更长的上文，超长时整次查询失效而不是截断。
pub const CONTEXT_LIMIT_BYTES: usize = 1024;

/// 一次排序超过这个时间算一帧慢帧。
const SLOW_ORDERING: Duration = Duration::from_millis(120);

/// 连续这么多次慢帧后，本回合余下的时间关掉句子模型，保护低端设备。
const SLOW_STREAK_LIMIT: u8 = 3;

/// 一页最多几个候选：数字键 1-9 各选一个。
pub const MAX_PAGE_SIZE: usize = 9;

/// 五笔码表答出的完整码长。
const WUBI_CODE_LENGTH: usize = 4;

/// 网页支持的输入方案。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    Quanpin,
    Xiaohe,
    Ziranma,
    Wubi86,
}

impl Scheme {
    /// `"quanpin"`、`"xiaohe"`、`"ziranma"`、`"wubi86"`；其他名字返回 None。
    pub fn parse(name: &str) -> Option<Scheme> {
        match name {
            "quanpin" => Some(Scheme::Quanpin),
            "xiaohe" => Some(Scheme::Xiaohe),
            "ziranma" => Some(Scheme::Ziranma),
            "wubi86" => Some(Scheme::Wubi86),
            _ => None,
        }
    }

    fn scheme_type(self) -> SchemeType {
        match self {
            Scheme::Quanpin => SchemeType::Quanpin,
            Scheme::Xiaohe | Scheme::Ziranma => SchemeType::Shuangpin,
            Scheme::Wubi86 => SchemeType::Wubi,
        }
    }

    fn shuangpin_profile(self) -> ShuangpinProfileKind {
        match self {
            Scheme::Ziranma => ShuangpinProfileKind::Ziranma,
            Scheme::Quanpin | Scheme::Xiaohe | Scheme::Wubi86 => ShuangpinProfileKind::Xiaohe,
        }
    }

    fn is_wubi(self) -> bool {
        self == Scheme::Wubi86
    }
}

/// 一次按键。JS 侧把它打包成 `(kind << 8) | ascii` 的 u32，见 [`Key::pack`]。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// `a`-`z`
    Letter(u8),
    /// `A`-`Z`（按着 Shift 或开着 CapsLock）
    ShiftLetter(u8),
    /// `b'0'..=b'9'`
    Digit(u8),
    Space,
    Enter,
    Backspace {
        word: bool,
    },
    Escape,
    /// 上一页。`punct` 是这次翻页借用的标点键（只能是 `b'-'`）：组字时照常翻页，空闲时打出这个标点；None 是 PageUp、方向键这类空闲时什么也不做的翻页键。
    PagePrev {
        punct: Option<u8>,
    },
    /// 下一页。`punct` 只能是 `b'='`，含义同 [`Key::PagePrev`]。
    PageNext {
        punct: Option<u8>,
    },
    HighlightPrev,
    HighlightNext,
    /// 不是字母也不是数字的可打印 ASCII，包括 `b'\''`。
    Punct(u8),
    /// 单独按下又松开 Shift。
    ShiftTap,
}

impl Key {
    /// 解包；未知的种类或不合种类的字节返回 None，调用方忽略它。
    pub fn unpack(packed: u32) -> Option<Key> {
        let kind = packed >> 8;
        let byte = (packed & 0xff) as u8;
        let bare = |key: Key| (byte == 0).then_some(key);
        // 翻页键的低字节为 0（PageUp、方向键），或者是它借用的那个标点键。
        let paging = |mark: u8| match byte {
            0 => Some(None),
            _ if byte == mark => Some(Some(mark)),
            _ => None,
        };
        match kind {
            1 if byte.is_ascii_lowercase() => Some(Key::Letter(byte)),
            2 if byte.is_ascii_uppercase() => Some(Key::ShiftLetter(byte)),
            3 if byte.is_ascii_digit() => Some(Key::Digit(byte)),
            4 => bare(Key::Space),
            5 => bare(Key::Enter),
            6 => bare(Key::Backspace { word: false }),
            7 => bare(Key::Backspace { word: true }),
            8 => bare(Key::Escape),
            9 => paging(b'-').map(|punct| Key::PagePrev { punct }),
            10 => paging(b'=').map(|punct| Key::PageNext { punct }),
            11 => bare(Key::HighlightPrev),
            12 => bare(Key::HighlightNext),
            13 if is_punct(byte) => Some(Key::Punct(byte)),
            14 => bare(Key::ShiftTap),
            _ => None,
        }
    }

    /// [`Key::unpack`] 的逆运算。
    pub fn pack(self) -> u32 {
        let (kind, byte): (u32, u8) = match self {
            Key::Letter(byte) => (1, byte),
            Key::ShiftLetter(byte) => (2, byte),
            Key::Digit(byte) => (3, byte),
            Key::Space => (4, 0),
            Key::Enter => (5, 0),
            Key::Backspace { word: false } => (6, 0),
            Key::Backspace { word: true } => (7, 0),
            Key::Escape => (8, 0),
            Key::PagePrev { punct } => (9, punct.unwrap_or(0)),
            Key::PageNext { punct } => (10, punct.unwrap_or(0)),
            Key::HighlightPrev => (11, 0),
            Key::HighlightNext => (12, 0),
            Key::Punct(byte) => (13, byte),
            Key::ShiftTap => (14, 0),
        };
        (kind << 8) | u32::from(byte)
    }
}

fn is_punct(byte: u8) -> bool {
    byte.is_ascii_graphic() && !byte.is_ascii_alphanumeric()
}

/// 一次调用产生的输出，按发生顺序排列。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Out {
    /// 上屏。`seat` 是候选在排序后列表里的 0 起座位（标点结束组字时，被结束的那部分也带座位）；原文上屏、标点本身以及五笔顶字和四码唯一自动上屏为 -1。
    Commit { text: String, seat: i32 },
    /// 空闲时直接打出的文字：英文模式下的字母、数字、空格、引擎不翻译的 ASCII 标点。
    Type(String),
    /// 空闲时的退格：删除已上屏的文字。
    Back { word: bool },
    /// 空闲时的 Esc，含义由 JS 侧决定。
    Exit,
}

/// 候选页上的一行。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub text: String,
    pub code: String,
}

/// 一次调用结束时的完整状态，JS 侧照着它画候选条。
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub out: Vec<Out>,
    pub composing: bool,
    /// `SessionSnapshot::preedit`
    pub preedit: String,
    /// `SessionSnapshot::caret_position`
    pub caret: u32,
    /// 当前页，最多 `page_size` 行。
    pub page: Vec<Row>,
    pub page_index: u32,
    pub has_prev: bool,
    /// 本页之后还有候选，或者这是最后一页而这次组字还没尝试过展开。
    pub has_next: bool,
    /// 页内下标。
    pub highlight: u32,
    /// 五笔：四个码表字母却没有任何候选（空码）。
    pub empty_code: bool,
    /// ShiftTap 切到了英文模式。
    pub english: bool,
    /// 句子模型已加载且没有被慢帧熔断关掉。
    pub model_on: bool,
    /// 这次调用里花在排序上的时间（没有排序时为 0）。
    pub rerank_ms: f64,
}

/// 排序后的候选状态。`engine_order[seat]` 是座位 `seat` 上的候选在引擎里的下标。
#[derive(Default)]
struct Ordered {
    rows: Vec<Row>,
    engine_order: Vec<usize>,
    /// 排序时的组字原文，用来判断列表是不是还是同一次组字。
    editing_text: String,
}

/// 网页输入法的一个会话。
pub struct WebHost {
    session: Session,
    options: SessionOptions,
    scheme: Scheme,
    page_size: usize,
    reranker: Option<Reranker>,
    /// 玩家是否希望开着模型（`set_model_enabled`）。
    model_wanted: bool,
    /// 慢帧熔断已触发，本回合不再用模型。
    model_tripped: bool,
    slow_streak: u8,
    /// 评测用：关掉熔断，让机器负载不影响测出来的排序。
    breaker_enabled: bool,
    /// 只含本会话自己输出过的文字（上屏和直通的 `Type`），从不读取目标文章（D18）。页面拒收的字（出错时停下打开时打错的字）也留在这里：引擎不知道页面收了哪些，上文因此可能和屏幕上的字不一致，引号配对随之可能给错一边。
    context: String,
    /// 页面的空闲退格是否真的删字（`set_backspace_deletes`）；出错时停下打开时页面不删，上文也就不能删。
    backspace_deletes: bool,
    english: bool,
    /// 最近一次快照；None 表示引擎状态变了还没重新取。
    snapshot: Option<SessionSnapshot>,
    /// 快照过期时，是否已知正在组字（字母键处理后一定在组字，不必为路由下一个键专门取快照）。
    composing_hint: Option<bool>,
    ordered: Ordered,
    /// `ordered` 是否对应当前快照。
    order_valid: bool,
    /// 上次排序之后改变引擎状态的次数。`Runtime::refresh` 每个键都比较一次，惰性排序（D19）只比较批首和批末；中间改过不止一次时，批首批末相同也不代表组字没换过（上屏后重打同一个码，退格后补回同一个字母）。
    engine_changes: u32,
    /// 当前高亮的座位（整个列表里的下标，不是页内下标）。
    highlighted: usize,
    /// 这次组字已经尝试过展开被截留的候选。
    expanded: bool,
    out: Vec<Out>,
    rerank_ms: f64,
}

impl WebHost {
    /// 主库必须已在 /res/msime.db（wasm 上由 import_database 放入；原生测试用真实路径构造，见 new_with_paths）
    pub fn new(scheme: Scheme, page_size: usize, model: Option<&[u8]>) -> Result<WebHost, String> {
        let resources = PathBuf::from("/res");
        let scratch = PathBuf::from("/scratch");
        let paths = RuntimePaths {
            resources: resources.clone(),
            user_data: scratch.clone(),
            cache: scratch,
            dictionaries: resources,
        };
        WebHost::with_paths(scheme, page_size, model, paths)
    }

    /// 原生测试入口：resources/dictionaries 指向 dir；user 与 cache 是两个独立的绝对路径临时目录（RuntimePaths::validate 要求绝对根，paths.rs:35-48）
    pub fn new_with_paths(
        scheme: Scheme,
        page_size: usize,
        model: Option<&[u8]>,
        dir: &Path,
        user: &Path,
        cache: &Path,
    ) -> Result<WebHost, String> {
        let paths = RuntimePaths {
            resources: dir.to_path_buf(),
            user_data: user.to_path_buf(),
            cache: cache.to_path_buf(),
            dictionaries: dir.to_path_buf(),
        };
        WebHost::with_paths(scheme, page_size, model, paths)
    }

    fn with_paths(
        scheme: Scheme,
        page_size: usize,
        model: Option<&[u8]>,
        paths: RuntimePaths,
    ) -> Result<WebHost, String> {
        if !(1..=MAX_PAGE_SIZE).contains(&page_size) {
            return Err(format!(
                "page size must be between 1 and {MAX_PAGE_SIZE}, got {page_size}"
            ));
        }
        let options = session_options(scheme, paths);
        let session = open_session(&options)?;
        // 五笔不重排（D16），所以不加载模型；模型读不出来也不致命，只是没有重排。
        let reranker = model
            .filter(|_| !scheme.is_wubi())
            .and_then(|bytes| SentenceModel::load(bytes).ok())
            .map(|model| Reranker::new(Arc::new(model)));
        Ok(WebHost {
            session,
            options,
            scheme,
            page_size,
            reranker,
            model_wanted: true,
            model_tripped: false,
            slow_streak: 0,
            breaker_enabled: true,
            context: String::new(),
            backspace_deletes: true,
            english: false,
            snapshot: None,
            composing_hint: Some(false),
            ordered: Ordered::default(),
            order_valid: false,
            engine_changes: 0,
            highlighted: 0,
            expanded: false,
            out: Vec::new(),
            rerank_ms: 0.0,
        })
    }

    /// 依次处理整批按键，中间状态不排序；返回批末的 Frame（out 汇总整批）
    pub fn keys(&mut self, keys: &[Key]) -> Frame {
        self.begin();
        for &key in keys {
            self.key(key);
        }
        self.frame()
    }

    /// 鼠标点击当前页第 slot 个候选
    pub fn pick(&mut self, slot: usize) -> Frame {
        self.begin();
        if slot < self.page_size && self.composing() {
            self.ensure_ordered();
            let seat = self.page_start() + slot;
            if seat < self.ordered.rows.len() {
                self.select_seat(seat, seat_number(seat));
            }
        }
        self.frame()
    }

    /// 关闭模型（玩家手动或熔断）
    pub fn set_model_enabled(&mut self, on: bool) {
        self.model_wanted = on;
        if on {
            // 玩家重新打开时给模型一次新机会。
            self.model_tripped = false;
            self.slow_streak = 0;
        }
        self.order_valid = false;
    }

    /// 页面的退格是否真的删字。出错时停下打开时页面的退格什么也不删，这时空闲退格只输出 `Out::Back`，不动上文；默认删。
    pub fn set_backspace_deletes(&mut self, deletes: bool) {
        self.backspace_deletes = deletes;
    }

    /// 取消组字、清空上下文，并重建 Session（标点交替状态随之归零），新回合从开引号开始
    pub fn reset(&mut self) -> Frame {
        self.begin();
        match open_session(&self.options) {
            Ok(session) => self.session = session,
            // 重建失败（库在这期间被删掉）时退回到取消组字，保住还能用的会话。
            Err(_) => {
                self.session.command(Command::Cancel);
            }
        }
        self.context.clear();
        self.session.set_rescoring_context("");
        self.english = false;
        self.model_tripped = false;
        self.slow_streak = 0;
        self.highlighted = 0;
        self.expanded = false;
        self.ordered = Ordered::default();
        self.invalidate();
        self.composing_hint = Some(false);
        self.frame()
    }

    /// 仅供 parity.rs 与 web_article_eval 复现 convert_eval/article_eval 的逐例上文；bindings.rs 不导出（D18）
    #[doc(hidden)]
    pub fn seed_context_for_eval(&mut self, text: &str) {
        self.context.clear();
        self.push_context(text);
        self.order_valid = false;
    }

    /// 仅供评测：关掉慢帧熔断，让机器负载不改变测出来的排序；bindings.rs 不导出。
    #[doc(hidden)]
    pub fn disable_slow_frame_breaker_for_eval(&mut self) {
        self.breaker_enabled = false;
    }

    /// 当前上文，测试用来确认它只含输出过的文字。
    #[doc(hidden)]
    pub fn context_for_tests(&self) -> &str {
        &self.context
    }

    fn begin(&mut self) {
        self.out.clear();
        self.rerank_ms = 0.0;
    }

    fn key(&mut self, key: Key) {
        match key {
            Key::Letter(byte) => self.letter(byte),
            Key::ShiftLetter(byte) => {
                if self.composing() {
                    self.commit_raw();
                }
                self.type_text(char::from(byte).to_string());
            }
            Key::ShiftTap => {
                if self.composing() {
                    self.commit_raw();
                }
                self.english = !self.english;
            }
            Key::Digit(byte) => self.digit(byte),
            Key::Space => {
                if self.composing() {
                    self.ensure_ordered();
                    if self.ordered.rows.is_empty() {
                        let result = self.session.command(Command::CommitCandidate);
                        self.apply(result, -1);
                    } else {
                        self.select_seat(self.highlighted, seat_number(self.highlighted));
                    }
                } else {
                    self.type_text(" ".to_owned());
                }
            }
            // 空闲的回车什么也不输出：跟打页面从不需要引擎打出换行。
            Key::Enter => {
                if self.composing() {
                    self.commit_raw();
                }
            }
            Key::Backspace { word } => {
                if self.composing() {
                    let result = self.session.command(Command::Backspace);
                    self.apply(result, -1);
                } else {
                    if self.backspace_deletes {
                        self.pop_context(word);
                    }
                    self.out.push(Out::Back { word });
                }
            }
            Key::Escape => {
                if self.composing() {
                    let result = self.session.command(Command::Cancel);
                    self.apply(result, -1);
                } else {
                    self.out.push(Out::Exit);
                }
            }
            Key::PageNext { punct } => {
                if self.composing() {
                    self.page_next();
                } else if let Some(byte) = punct {
                    self.punct(byte);
                }
            }
            Key::PagePrev { punct } => {
                if self.composing() {
                    self.ensure_ordered();
                    if !self.ordered.rows.is_empty() {
                        let page = self.highlighted / self.page_size;
                        self.highlighted = page.saturating_sub(1) * self.page_size;
                    }
                } else if let Some(byte) = punct {
                    self.punct(byte);
                }
            }
            Key::HighlightNext => self.highlight_next(),
            Key::HighlightPrev => {
                if self.composing() {
                    self.ensure_ordered();
                    if !self.ordered.rows.is_empty() {
                        self.highlighted = self.highlighted.saturating_sub(1);
                    }
                }
            }
            Key::Punct(byte) => self.punct(byte),
        }
    }

    fn letter(&mut self, byte: u8) {
        if self.english {
            self.type_text(char::from(byte).to_string());
            return;
        }
        if self.scheme.is_wubi() {
            self.wubi_letter(byte);
            return;
        }
        let composing = self.composing();
        let result = self.session.character(byte, false);
        if result.commit.is_some() {
            self.apply(result, -1);
        } else if result.handled {
            // 只标记过期，排序留到这批按键结束或下一个要用座位的键之前（D19）。
            self.invalidate();
            self.composing_hint = Some(true);
        } else if !composing {
            // 组字中引擎不收的字母什么也不做；空闲时原样打出。
            self.type_text(char::from(byte).to_string());
        }
    }

    /// 五笔字母：先看顶字（`Runtime::dispatch` 的 `wubi_top_commit`），再看四码唯一自动上屏。
    fn wubi_letter(&mut self, byte: u8) {
        let composing = self.composing();
        // 五笔码表只用 a-y；`z` 在空闲时原样打出。
        if !composing && byte == b'z' {
            self.type_text("z".to_owned());
            return;
        }
        if composing && wubi_code_complete(self.ensure_snapshot()) {
            // 完整的四码后又来一个字母：先上屏首选，再用这个字母开始新的码。
            self.select_seat(0, -1);
        }
        let idle_before = !self.composing();
        let result = self.session.character(byte, false);
        let handled = result.handled;
        if result.commit.is_some() {
            self.apply(result, -1);
        } else if handled {
            self.invalidate();
        } else if idle_before {
            self.type_text(char::from(byte).to_string());
            return;
        }
        if self.ensure_snapshot().wubi_unique_four_code {
            self.select_seat(0, -1);
        }
    }

    fn digit(&mut self, byte: u8) {
        if self.english || !self.composing() {
            self.type_text(char::from(byte).to_string());
            return;
        }
        // 先让引擎看数字（`Runtime::dispatch`），它不收时 1-9 才是选字。
        let result = self.session.character(byte, false);
        if result.handled || result.commit.is_some() {
            self.apply(result, -1);
            return;
        }
        if byte == b'0' {
            return;
        }
        self.ensure_ordered();
        let slot = usize::from(byte - b'1');
        let seat = self.page_start() + slot;
        if slot < self.page_size && seat < self.ordered.rows.len() {
            self.select_seat(seat, seat_number(seat));
        }
    }

    fn punct(&mut self, byte: u8) {
        if self.english {
            self.type_text(char::from(byte).to_string());
            return;
        }
        let composing = self.composing();
        // 全拼里光标不在开头时，撇号是音节分隔符；引擎不收时按标点处理（`Runtime::dispatch` 里未处理的字符落到 `punctuation`）。
        if byte == b'\''
            && self.scheme == Scheme::Quanpin
            && composing
            && self.ensure_snapshot().caret_position > 0
        {
            let result = self.session.character(byte, false);
            if result.handled || result.commit.is_some() {
                self.apply(result, -1);
                return;
            }
        }
        let mut finished = false;
        if composing {
            // 先按高亮的座位结束组字，再翻译标点：在组字中直接调用 `session.punctuation` 会上屏引擎的第 0 个候选（`Runtime::punctuation`）。
            self.ensure_ordered();
            let seat = self.highlighted;
            let result = self.session.finish(self.engine_index(seat));
            finished = result
                .commit
                .as_deref()
                .is_some_and(|text| !text.is_empty());
            self.apply(result, seat_number(seat));
        }
        if let Some(mark) = quote_mark(&self.context, byte) {
            self.commit(mark.to_owned(), -1);
            return;
        }
        let result = self.session.punctuation(byte);
        match result.commit {
            Some(text) => {
                self.invalidate();
                self.commit(text, -1);
            }
            // 没有中文形式的 ASCII 标点：结束了组字时和组字一起上屏（`Runtime::punctuation`），空闲时原样打出。
            None if finished => self.commit(char::from(byte).to_string(), -1),
            None => self.type_text(char::from(byte).to_string()),
        }
    }

    fn page_next(&mut self) {
        self.ensure_ordered();
        let len = self.ordered.rows.len();
        if len == 0 {
            return;
        }
        // `Runtime::expand_for_next_page`：在最后一页，或下一页是不满的最后一页时，先要回被截留的候选；新来的候选填满了当前页时留在原页。
        let page = self.highlighted / self.page_size;
        let last_page = (len - 1) / self.page_size;
        let next_is_partial_last = page + 1 == last_page && !len.is_multiple_of(self.page_size);
        if page == last_page || next_is_partial_last {
            let page_was_full = (page + 1) * self.page_size <= len;
            if self.expand() && page == last_page && !page_was_full {
                return;
            }
        }
        let len = self.ordered.rows.len();
        self.highlighted = (page + 1).min((len - 1) / self.page_size) * self.page_size;
    }

    fn highlight_next(&mut self) {
        if !self.composing() {
            return;
        }
        self.ensure_ordered();
        let len = self.ordered.rows.len();
        if len == 0 {
            return;
        }
        // `Runtime::expand_for_next_candidate`：走到已加载的最后一个候选，或从页尾走进不满的最后一页时先展开。
        let page = self.highlighted / self.page_size;
        let last_page = (len - 1) / self.page_size;
        let at_last_candidate = self.highlighted + 1 == len;
        let at_page_end = (self.highlighted + 1).is_multiple_of(self.page_size);
        let next_is_partial_last = page + 1 == last_page && !len.is_multiple_of(self.page_size);
        if at_last_candidate || (at_page_end && next_is_partial_last) {
            self.expand();
        }
        let len = self.ordered.rows.len();
        self.highlighted = (self.highlighted + 1).min(len - 1);
    }

    /// 向引擎要回首次查询截留的候选（单字母查询只给前 24 个），并按当前上文重新排序；高亮保持不动。返回列表是否变长。
    fn expand(&mut self) -> bool {
        self.expanded = true;
        if !self.session.expand_initial_candidates() {
            return false;
        }
        self.snapshot = None;
        self.order_valid = false;
        self.order(true);
        true
    }

    fn commit_raw(&mut self) {
        let result = self.session.command(Command::CommitRaw);
        self.apply(result, -1);
    }

    fn select_seat(&mut self, seat: usize, number: i32) {
        self.ensure_ordered();
        let result = self.session.select(self.engine_index(seat));
        self.apply(result, number);
    }

    /// 座位越界时原样传给引擎，和 `Runtime::engine_index` 一样。
    fn engine_index(&self, seat: usize) -> usize {
        self.ordered.engine_order.get(seat).copied().unwrap_or(seat)
    }

    /// 处理一次改变了引擎状态的调用：有上屏就输出，并让缓存的快照和排序过期。
    fn apply(&mut self, result: KeyResult, seat: i32) {
        self.invalidate();
        if let Some(text) = result.commit {
            self.commit(text, seat);
        }
    }

    fn invalidate(&mut self) {
        self.engine_changes = self.engine_changes.saturating_add(1);
        self.snapshot = None;
        self.composing_hint = None;
        self.order_valid = false;
    }

    fn commit(&mut self, text: String, seat: i32) {
        if text.is_empty() {
            return;
        }
        self.push_context(&text);
        self.out.push(Out::Commit { text, seat });
    }

    fn type_text(&mut self, text: String) {
        self.push_context(&text);
        self.out.push(Out::Type(text));
    }

    /// `Runtime::remember_commit`：保留尾部 1024 字节，在字符边界处切，然后交给引擎。
    fn push_context(&mut self, text: &str) {
        self.context.push_str(text);
        if self.context.len() > CONTEXT_LIMIT_BYTES {
            let mut cut = self.context.len() - CONTEXT_LIMIT_BYTES;
            while !self.context.is_char_boundary(cut) {
                cut += 1;
            }
            self.context.drain(..cut);
        }
        self.session.set_rescoring_context(&self.context);
    }

    /// 空闲退格：删掉上文最后一个字；`word` 时删掉末尾连续的一段汉字（末尾不是汉字时只删一个字）。
    fn pop_context(&mut self, word: bool) {
        let Some(last) = self.context.pop() else {
            return;
        };
        if word && is_han(last) {
            while self.context.chars().next_back().is_some_and(is_han) {
                self.context.pop();
            }
        }
        self.session.set_rescoring_context(&self.context);
        self.order_valid = false;
    }

    fn composing(&mut self) -> bool {
        if self.snapshot.is_none() {
            if let Some(composing) = self.composing_hint {
                return composing;
            }
        }
        is_composing(self.ensure_snapshot())
    }

    fn ensure_snapshot(&mut self) -> &SessionSnapshot {
        self.snapshot.get_or_insert_with(|| self.session.snapshot())
    }

    fn ensure_ordered(&mut self) {
        if !self.order_valid {
            self.order(false);
        }
    }

    /// 排序刷新（D19）：句子模型挑首位，再把多余的整句读法后移，结果记成 `engine_order`。和 `Runtime::refresh` 的 `rerank` + `demote_runner_up_readings` 完全对应，`tests/parity.rs` 钉住这一点。
    fn order(&mut self, keep_highlight: bool) {
        let started = Instant::now();
        let use_model = self.model_on();
        let snapshot = self.snapshot.get_or_insert_with(|| self.session.snapshot());
        let scheme = snapshot.scheme as u8;
        let rows: Vec<OrderRow<'_>> = snapshot
            .candidates
            .iter()
            .enumerate()
            .map(|(index, item)| OrderRow {
                text: &item.word,
                source: item.source as u8,
                // 和 `host::Session::snapshot` 一样：缺的项按不回答整个按键处理。
                answers_key: snapshot
                    .candidate_answers_key
                    .get(index)
                    .copied()
                    .unwrap_or(false),
                corrected: !item.corrected_from.is_empty(),
            })
            .collect();
        let mut order: Vec<usize> = (0..rows.len()).collect();
        let mut ran_model = false;
        if use_model {
            if let Some(reranker) = self.reranker.as_mut() {
                ran_model = true;
                if let Some(promote) = rerank_pick(
                    reranker,
                    &self.context,
                    scheme,
                    snapshot.answered_by_pinyin_fallback,
                    &rows,
                ) {
                    rotate_to_front(&mut order, promote);
                }
            }
        }
        // `Runtime::demote_runner_up_readings` 看到的是已经旋转过的列表，这里按旋转后的顺序交给它。
        let rotated: Vec<OrderRow<'_>> = order.iter().map(|&index| rows[index]).collect();
        if let Some(demoted) = runner_up_order(scheme, &rotated) {
            order = demoted.iter().map(|&seat| order[seat]).collect();
        }
        let ordered_rows: Vec<Row> = order
            .iter()
            .map(|&index| {
                let item = &snapshot.candidates[index];
                Row {
                    text: item.word.clone(),
                    code: item.pinyin.clone(),
                }
            })
            .collect();
        let editing_text = snapshot.editing_text.clone();
        // `Runtime::refresh`：列表和组字都没变时高亮留在原处，否则回到首位；换了一次组字，展开也要重新来过。上次排序后引擎改过不止一次时，中间必定经过别的状态，`Runtime` 在那里已经回到首位。
        let same_composition =
            self.engine_changes <= 1 && editing_text == self.ordered.editing_text;
        self.engine_changes = 0;
        let unchanged = same_composition && ordered_rows == self.ordered.rows;
        if !same_composition {
            self.expanded = false;
        }
        if !keep_highlight && !unchanged {
            self.highlighted = 0;
        }
        self.highlighted = self.highlighted.min(ordered_rows.len().saturating_sub(1));
        self.ordered = Ordered {
            rows: ordered_rows,
            engine_order: order,
            editing_text,
        };
        self.order_valid = true;
        let elapsed = started.elapsed();
        self.rerank_ms += elapsed.as_secs_f64() * 1000.0;
        if ran_model && self.breaker_enabled {
            if elapsed > SLOW_ORDERING {
                self.slow_streak = self.slow_streak.saturating_add(1);
            } else {
                self.slow_streak = 0;
            }
            if self.slow_streak >= SLOW_STREAK_LIMIT {
                self.model_tripped = true;
            }
        }
    }

    fn model_on(&self) -> bool {
        self.reranker.is_some() && self.model_wanted && !self.model_tripped
    }

    fn page_start(&self) -> usize {
        self.highlighted / self.page_size * self.page_size
    }

    fn frame(&mut self) -> Frame {
        self.ensure_ordered();
        let wubi = self.scheme.is_wubi();
        let snapshot = self.ensure_snapshot();
        let composing = is_composing(snapshot);
        let preedit = snapshot.preedit.clone();
        let caret = u32::try_from(snapshot.caret_position).unwrap_or(u32::MAX);
        let empty_code = wubi
            && snapshot.editing_text.len() == WUBI_CODE_LENGTH
            && snapshot
                .editing_text
                .bytes()
                .all(|byte| byte.is_ascii_alphabetic())
            && snapshot.candidates.is_empty();
        let len = self.ordered.rows.len();
        let page = self.highlighted / self.page_size;
        let start = page * self.page_size;
        let end = (start + self.page_size).min(len);
        let last_page = len.saturating_sub(1) / self.page_size;
        Frame {
            out: std::mem::take(&mut self.out),
            composing,
            preedit,
            caret,
            page: self.ordered.rows[start.min(len)..end].to_vec(),
            page_index: u32::try_from(page).unwrap_or(u32::MAX),
            has_prev: page > 0,
            has_next: len > 0 && (page < last_page || !self.expanded),
            highlight: u32::try_from(self.highlighted - start).unwrap_or(0),
            empty_code,
            english: self.english,
            model_on: self.model_on(),
            rerank_ms: self.rerank_ms,
        }
    }
}

/// 座位号转成 `Out::Commit::seat`。
fn seat_number(seat: usize) -> i32 {
    i32::try_from(seat).unwrap_or(i32::MAX)
}

fn is_composing(snapshot: &SessionSnapshot) -> bool {
    !snapshot.editing_text.is_empty() || !snapshot.preedit.is_empty()
}

/// `input_runtime::wubi_four_code_is_complete`：码表答出的完整四码，光标在末尾，且有候选可上屏。
fn wubi_code_complete(snapshot: &SessionSnapshot) -> bool {
    snapshot.scheme == SchemeType::Wubi
        && !snapshot.dedicated_english
        && snapshot.local_mode == LocalInputMode::None
        && !snapshot.answered_by_pinyin_fallback
        && snapshot.editing_text.len() == WUBI_CODE_LENGTH
        && snapshot
            .editing_text
            .bytes()
            .all(|byte| byte.is_ascii_alphabetic())
        && snapshot.caret_position == WUBI_CODE_LENGTH
        && !snapshot.candidates.is_empty()
}

/// 网页自己的引号规则：引擎的 `PunctuationPolicy` 状态无法回退（空闲退格删掉 `“` 后，引擎下一次会给 `”`），所以 `"`、`'`、`<`、`>` 从不交给引擎，而是由上文推出来。对格式正确的文字，结果和 `punctuation.rs` 的交替与嵌套一致。其他键返回 None。
fn quote_mark(context: &str, key: u8) -> Option<&'static str> {
    let count = |mark: char| context.chars().filter(|&ch| ch == mark).count();
    match key {
        b'"' => Some(if count('“') <= count('”') {
            "“"
        } else {
            "”"
        }),
        b'\'' => Some(if count('‘') <= count('’') {
            "‘"
        } else {
            "’"
        }),
        b'<' | b'>' => {
            // 还没闭合的书名号，按出现顺序。
            let mut open: Vec<char> = Vec::new();
            for ch in context.chars() {
                match ch {
                    '《' | '〈' => open.push(ch),
                    '》' | '〉' => {
                        let opening = if ch == '》' { '《' } else { '〈' };
                        if let Some(at) = open.iter().rposition(|&mark| mark == opening) {
                            open.truncate(at);
                        }
                    }
                    _ => {}
                }
            }
            Some(if key == b'<' {
                if open.contains(&'《') {
                    "〈"
                } else {
                    "《"
                }
            } else if open.last() == Some(&'〈') && open.contains(&'《') {
                "〉"
            } else {
                "》"
            })
        }
        _ => None,
    }
}

fn is_han(ch: char) -> bool {
    matches!(ch,
        '\u{3400}'..='\u{4DBF}'
        | '\u{4E00}'..='\u{9FFF}'
        | '\u{F900}'..='\u{FAFF}'
        | '\u{20000}'..='\u{2FA1F}'
        | '〇')
}

fn session_options(scheme: Scheme, paths: RuntimePaths) -> SessionOptions {
    let mut options = SessionOptions::new(paths);
    options.scheme = scheme.scheme_type();
    options.shuangpin_profile = scheme.shuangpin_profile();
    options.wubi = WubiInputOptions {
        mixed_pinyin: false,
        profile: WubiProfileKind::Wubi86,
    };
    // 学习、个人上文（它会起一个在 wasm 上 panic 的落盘线程）、辅助码、所有本地模式（日期时间模式在 wasm 上读本地时间会 panic）、混输英文、模糊音和纠错都关掉，和测出 40.9% 的评测配置一致（D15）。
    options.learning = false;
    options.personal_context = false;
    options.helpcode = false;
    options.local_modes = LocalModeOptions {
        unicode: false,
        date_time: false,
        quick_phrase: false,
        emoji: false,
        kaomoji: false,
        super_jianpin: false,
        temporary_english: false,
        temporary_japanese: false,
        expression: false,
        command: false,
        mention: false,
    };
    options.english = EnglishInputOptions {
        mixed_candidates: false,
        ..EnglishInputOptions::default()
    };
    options.expressive = MixedExpressiveOptions::default();
    options.fuzzy_pinyin = FuzzyPinyinOptions::default();
    options.autocorrect_types = 0;
    options.frequency = FrequencyAdjustmentOptions::default();
    options.sentence_association = SentenceAssociationOptions {
        neural_keyboard: false,
        ..SentenceAssociationOptions::default()
    };
    // 和 host-api 一样要所有整句读法，由 `runner_up_order` 自己裁剪。
    options.sentence_alternatives = true;
    options.chinese_punctuation = true;
    options.paired_punctuation = false;
    options
}

fn open_session(options: &SessionOptions) -> Result<Session, String> {
    let mut session = Session::new(options.clone()).map_err(|error| error.to_string())?;
    session.set_chinese_punctuation_enabled(true);
    // 引擎的成对标点只影响宿主自动补全后的嵌套计数，网页不做自动补全，所以关掉才是如实的设置。
    session.set_paired_punctuation_enabled(false);
    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip_through_the_packed_form() {
        let keys = [
            Key::Letter(b'a'),
            Key::Letter(b'z'),
            Key::ShiftLetter(b'Q'),
            Key::Digit(b'0'),
            Key::Digit(b'9'),
            Key::Space,
            Key::Enter,
            Key::Backspace { word: false },
            Key::Backspace { word: true },
            Key::Escape,
            Key::PagePrev { punct: None },
            Key::PageNext { punct: None },
            Key::PagePrev { punct: Some(b'-') },
            Key::PageNext { punct: Some(b'=') },
            Key::HighlightPrev,
            Key::HighlightNext,
            Key::Punct(b'\''),
            Key::Punct(b','),
            Key::Punct(b'~'),
            Key::ShiftTap,
        ];
        for key in keys {
            assert_eq!(Key::unpack(key.pack()), Some(key), "{key:?}");
        }
        assert_eq!(Key::pack(Key::Letter(b'n')), (1 << 8) | u32::from(b'n'));
        assert_eq!(Key::pack(Key::ShiftTap), 14 << 8);
        assert_eq!(Key::pack(Key::PagePrev { punct: None }), 9 << 8);
        assert_eq!(
            Key::pack(Key::PageNext { punct: Some(b'=') }),
            (10 << 8) | u32::from(b'=')
        );
    }

    #[test]
    fn unknown_kinds_and_bytes_are_ignored() {
        assert_eq!(Key::unpack(0), None);
        assert_eq!(Key::unpack(15 << 8), None);
        assert_eq!(Key::unpack((1 << 8) | u32::from(b'A')), None);
        assert_eq!(Key::unpack((2 << 8) | u32::from(b'a')), None);
        assert_eq!(Key::unpack((3 << 8) | u32::from(b'x')), None);
        assert_eq!(Key::unpack((4 << 8) | 1), None);
        // 翻页键只认自己借用的那个标点。
        assert_eq!(Key::unpack((9 << 8) | u32::from(b'=')), None);
        assert_eq!(Key::unpack((10 << 8) | u32::from(b'-')), None);
        assert_eq!(Key::unpack((9 << 8) | u32::from(b',')), None);
        assert_eq!(Key::unpack((13 << 8) | u32::from(b'a')), None);
        assert_eq!(Key::unpack((13 << 8) | u32::from(b' ')), None);
        assert_eq!(Key::unpack((13 << 8) | 0x7f), None);
        assert_eq!(Key::unpack((13 << 8) | 0xa1), None);
    }

    #[test]
    fn schemes_parse_by_name() {
        assert_eq!(Scheme::parse("quanpin"), Some(Scheme::Quanpin));
        assert_eq!(Scheme::parse("xiaohe"), Some(Scheme::Xiaohe));
        assert_eq!(Scheme::parse("ziranma"), Some(Scheme::Ziranma));
        assert_eq!(Scheme::parse("wubi86"), Some(Scheme::Wubi86));
        assert_eq!(Scheme::parse("wubi98"), None);
        assert_eq!(Scheme::parse(""), None);
    }

    #[test]
    fn quotes_alternate_from_the_context() {
        assert_eq!(quote_mark("", b'"'), Some("“"));
        assert_eq!(quote_mark("他说“好", b'"'), Some("”"));
        assert_eq!(quote_mark("“好”", b'"'), Some("“"));
        assert_eq!(quote_mark("", b'\''), Some("‘"));
        assert_eq!(quote_mark("‘a", b'\''), Some("’"));
        assert_eq!(quote_mark("", b','), None);
    }

    #[test]
    fn book_titles_nest_like_the_engine_policy() {
        let mut context = String::new();
        let mut typed = String::new();
        for key in *b"<<>>" {
            let mark = quote_mark(&context, key).unwrap();
            context.push_str(mark);
            typed.push_str(mark);
        }
        assert_eq!(typed, "《〈〉》");
        // 没有配对的 `>` 给外层的 `》`，下一个 `<` 仍然开外层。
        assert_eq!(quote_mark("", b'>'), Some("》"));
        assert_eq!(quote_mark("》", b'<'), Some("《"));
        assert_eq!(quote_mark("《书》", b'<'), Some("《"));
        assert_eq!(quote_mark("《书〈章", b'>'), Some("〉"));
        assert_eq!(quote_mark("《书〈章〉", b'>'), Some("》"));
    }

    #[test]
    fn han_detection_covers_the_common_blocks() {
        assert!(is_han('你'));
        assert!(is_han('〇'));
        assert!(is_han('\u{20000}'));
        assert!(!is_han('a'));
        assert!(!is_han('，'));
    }
}
