//! The tools, and the limits around the ones that write.
//!
//! Every tool reads the runtime-options document afresh, so the server follows the settings page when it moves the data directory or changes the dictionaries without being restarted. Nothing typed and no credential leaves the input method through here, and no dictionary content beyond quick phrases unless the user started the server with --allow-dictionary-read. The diagnostic log is always readable: it holds event names, timings and error codes, the user turns it on themselves, and the keys the Windows TIP traces are blanked before a line is returned; the audit lines on stderr name the tool and the outcome, never what was written.

use crate::config::Config;
use crate::diagnostics::{self, LogRequest, LogView, SwitchRequest, SwitchView};
use crate::preferences::{self, PreferencesChange, PreferencesView};
use crate::prompts::WRITE_PROMPTS;
use crate::skins::{self, CreateSkinRequest, CreatedSkin, SkinList};
use crate::statistics::{self, StatisticsRequest, StatisticsView};
use crate::words;
use crate::words::{
    LookupRequest, LookupView, WordEditRequest, WordImportOutcome, WordImportRequest, WordListPage,
    WordListRequest,
};
use msime_client_core::dictionary::quiesce::QuiescedHosts;
use msime_host_api::{DictionaryOptions, QuickPhrase, QuickPhraseEdit, WordEdit};
use rmcp::handler::server::router::prompt::PromptRouter;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::schemars::JsonSchema;
use rmcp::{prompt_handler, tool, tool_handler, tool_router, Json, ServerHandler};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const DEFAULT_PAGE: usize = 100;
const MAX_PAGE: usize = 1000;
/// The most edits one call may carry. An agent tidying a handful of phrases stays well inside it; one rewriting the whole collection has to show its work across calls.
const MAX_EDITS: usize = 50;
/// The shortest gap between two writing calls, so an agent stuck in a loop cannot rewrite the dictionary or the preferences many times a second.
const WRITE_INTERVAL: Duration = Duration::from_secs(1);

const WRITE_TOOLS: [&str; 3] = [
    "create_candidate_skin",
    "edit_quick_phrases",
    "update_preferences",
];
/// Offered with --allow-dictionary-read.
const DICTIONARY_READ_TOOLS: [&str; 2] = ["list_dictionary_words", "lookup_candidates"];
/// Offered with --allow-write and --allow-dictionary-read together: an edit or import also tells whether a word is there.
const DICTIONARY_WRITE_TOOLS: [&str; 2] = ["edit_dictionary_words", "import_dictionary_words"];

const INSTRUCTIONS: &str = "Manages 水杉输入法 (MSIME), a Chinese input method: its quick phrases (a short code the user types that expands to a longer text), a few of its preferences, and aggregate typing statistics. With --allow-dictionary-read it also lists the user's own dictionary words and shows which candidates a code offers and why, which is how to explain a candidate's rank; with --allow-write as well it adds, reweights, removes and imports words. To import a word list the user gives you, read it yourself and send the words, 200 at a time. It also helps with problems the user runs into: lag, a candidate window that is missing or in the wrong place, the input method stopping or switching by itself. The user may not be technical, so do the steps yourself rather than asking them to open files, settings or a terminal: read the log with read_diagnostic_log; if it is off, turn it on with set_diagnostic_log, ask the user in plain words to do again what went wrong and to tell you when they have, then read the log again and explain what you found in plain words. Turn the log off with set_diagnostic_log when you are done. It also lists the user's candidate-window skins and, with --allow-write, makes new ones: write a skin.toml and PNG or JPEG images yourself and pass them to create_candidate_skin. The desktop app then syncs every skin made this way to the user's cloud library as a private package, from where the user can publish it to the community; this server never signs in to the account itself. Changes take effect in the input method within a few seconds. Apart from set_diagnostic_log, writing tools are only offered when the user started the server with --allow-write.";

#[derive(Clone)]
pub struct MsimeServer {
    config: Arc<Config>,
    last_write: Arc<Mutex<Option<Instant>>>,
    /// Set while a write runs. rmcp runs each request as its own task and a write can outlast the interval, so spacing alone would let two overlap on the quiesce lease and on a check-then-write edit.
    writing: Arc<AtomicBool>,
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct ListRequest {
    /// Only phrases whose code starts with this. Codes are lowercase.
    pub code_prefix: Option<String>,
    /// How many matching phrases to skip.
    pub offset: Option<usize>,
    /// At most this many phrases, 1 to 1000. Defaults to 100.
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct Phrase {
    /// What the user types, lowercase letters.
    pub code: String,
    /// What it expands to.
    pub text: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
pub struct PhrasePage {
    pub phrases: Vec<Phrase>,
    /// More phrases match; ask again with a larger offset.
    pub has_more: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Edit {
    /// Add a phrase. Refused if the same code already expands to the same text.
    Add { code: String, text: String },
    /// Change a phrase, found by its current code and text, keeping its rank.
    Replace {
        previous: Phrase,
        replacement: Phrase,
    },
    /// Remove a phrase, found by its code and text.
    Remove { code: String, text: String },
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct EditRequest {
    /// Applied in order, at most 50. The first that fails stops the rest; those before it stay applied.
    pub edits: Vec<Edit>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct EditOutcome {
    /// How many edits were applied, from the start of the list.
    pub applied: usize,
    /// The index of the edit that failed, when one did.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl From<QuickPhrase> for Phrase {
    fn from(value: QuickPhrase) -> Self {
        Self {
            code: value.code,
            text: value.text,
        }
    }
}

impl From<Phrase> for QuickPhrase {
    fn from(value: Phrase) -> Self {
        Self {
            code: value.code,
            text: value.text,
        }
    }
}

impl From<Edit> for QuickPhraseEdit {
    fn from(value: Edit) -> Self {
        match value {
            Edit::Add { code, text } => Self::Add(QuickPhrase { code, text }),
            Edit::Replace {
                previous,
                replacement,
            } => Self::Replace {
                previous: previous.into(),
                replacement: replacement.into(),
            },
            Edit::Remove { code, text } => Self::Remove(QuickPhrase { code, text }),
        }
    }
}

#[tool_router]
impl MsimeServer {
    pub fn new(config: Config) -> Self {
        let mut tool_router = Self::tool_router();
        let hidden = [
            (!config.allow_write, &WRITE_TOOLS[..]),
            (!config.allow_dictionary_read, &DICTIONARY_READ_TOOLS[..]),
            (
                !(config.allow_write && config.allow_dictionary_read),
                &DICTIONARY_WRITE_TOOLS[..],
            ),
        ];
        for name in hidden
            .into_iter()
            .filter(|(hide, _)| *hide)
            .flat_map(|(_, names)| names)
        {
            tool_router.remove_route(name);
        }
        let mut prompt_router = Self::prompt_router();
        if !config.allow_write {
            for name in WRITE_PROMPTS {
                prompt_router.remove_route(name);
            }
        }
        Self {
            config: Arc::new(config),
            last_write: Arc::new(Mutex::new(None)),
            writing: Arc::new(AtomicBool::new(false)),
            tool_router,
            prompt_router,
        }
    }

    #[tool(
        name = "list_quick_phrases",
        description = "List the user's own quick phrases: short codes that expand to longer text when typed. Built-in phrases are not included.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_quick_phrases(
        &self,
        Parameters(request): Parameters<ListRequest>,
    ) -> Result<Json<PhrasePage>, String> {
        let limit = request.limit.unwrap_or(DEFAULT_PAGE);
        if !(1..=MAX_PAGE).contains(&limit) {
            return Err(format!("limit must be between 1 and {MAX_PAGE}"));
        }
        let config = self.config.clone();
        blocking(move || {
            let options = DictionaryOptions::from_host_document(config.read_host_options()?)?;
            let page = msime_host_api::user_quick_phrases(
                &options,
                request.code_prefix.as_deref().unwrap_or(""),
                request.offset.unwrap_or(0),
                limit,
            )?;
            Ok(Json(PhrasePage {
                phrases: page.phrases.into_iter().map(Phrase::from).collect(),
                has_more: page.has_more,
            }))
        })
        .await
    }

    #[tool(
        name = "edit_quick_phrases",
        description = "Add, replace or remove the user's quick phrases. Each phrase is found by its exact code and text, so list them first. Edits are applied in order and the first failure stops the rest.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn edit_quick_phrases(
        &self,
        Parameters(request): Parameters<EditRequest>,
    ) -> Result<Json<EditOutcome>, String> {
        if request.edits.is_empty() || request.edits.len() > MAX_EDITS {
            return Err(format!("send between 1 and {MAX_EDITS} edits"));
        }
        let guard = self.claim_write()?;
        let config = self.config.clone();
        let edits: Vec<QuickPhraseEdit> = request.edits.into_iter().map(Into::into).collect();
        let outcome = blocking(move || {
            let _guard = guard;
            let options = DictionaryOptions::from_host_document(config.read_host_options()?)?;
            Ok(apply_edits(
                &options,
                &edits,
                msime_host_api::edit_user_quick_phrase,
            ))
        })
        .await?;
        eprintln!(
            "msime-mcp: edit_quick_phrases applied {}{}",
            outcome.applied,
            if outcome.error.is_some() {
                " then failed"
            } else {
                ""
            }
        );
        Ok(Json(outcome))
    }

    #[tool(
        name = "get_preferences",
        description = "Read the preferences an agent may see: input scheme, shuangpin layout, candidate page size, font size and layout, the starting mode, character width, punctuation, Traditional Chinese output, Wubi switches and a few more. Pass the returned revision to update_preferences.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn get_preferences(&self) -> Result<Json<PreferencesView>, String> {
        let config = self.config.clone();
        blocking(move || {
            let state_dir = config.state_dir(&config.read_options()?)?;
            preferences::load(&state_dir).map(Json)
        })
        .await
    }

    #[tool(
        name = "update_preferences",
        description = "Change some of the preferences get_preferences returns. Only the fields given change. Refused when the preferences changed since expected_revision was read.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn update_preferences(
        &self,
        Parameters(change): Parameters<PreferencesChange>,
    ) -> Result<Json<PreferencesView>, String> {
        // Refused before the slot is taken, so a mistaken call does not hold up the corrected one.
        if change.is_empty() {
            return Err("no preference to change".into());
        }
        let guard = self.claim_write()?;
        let config = self.config.clone();
        let result = blocking(move || {
            let _guard = guard;
            let state_dir = config.state_dir(&config.read_options()?)?;
            preferences::update(&state_dir, &config.options, &change).map(Json)
        })
        .await;
        eprintln!(
            "msime-mcp: update_preferences {}",
            if result.is_ok() { "saved" } else { "refused" }
        );
        result
    }

    #[tool(
        name = "list_candidate_skins",
        description = "List the candidate-window skins installed as folders in the input method's skin directory: id, name, version, the layouts and colour modes each supports, and whether the desktop app has synced it to the user's cloud library. Folders that are not a usable skin are listed with the reason.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_candidate_skins(&self) -> Result<Json<SkinList>, String> {
        let config = self.config.clone();
        blocking(move || {
            let state_dir = config.state_dir(&config.read_options()?)?;
            Ok(Json(skins::list(&state_dir)))
        })
        .await
    }

    #[tool(
        name = "create_candidate_skin",
        description = "Install a candidate-window skin from a skin.toml and its images. The manifest is TOML: schema_version = 1; id (equal to package_id); name (at most 80 bytes); version; base, the built-in theme drawn under it (system, shuishan, light, paper, night or ink); preview, the path of a PNG or JPEG shown in the skin list; optional author, description and [license] code, assets and source; [supports] layouts (horizontal, vertical) and themes (dark, light); [candidate_window] min_width_dip (0-1000), optional corner_radius_dip (0-32), optional [candidate_window.decoration] top_inset_dip, width_dip, image and align (left, center, right), optional [candidate_window.background] image, fit (cover, contain, stretch) and opacity (0-1); [candidate.dark] and [candidate.light] colours accent, selected, hover, surface, border, text, number and translation as #RRGGBB or #RRGGBBAA, plus show_selected_bar. images must hold exactly the images the manifest references. A stylesheet is not accepted, so the skin can be synced and shared. Refused when the skin exists, unless replace is true.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn create_candidate_skin(
        &self,
        Parameters(request): Parameters<CreateSkinRequest>,
    ) -> Result<Json<CreatedSkin>, String> {
        let guard = self.claim_write()?;
        let config = self.config.clone();
        let result = blocking(move || {
            let _guard = guard;
            let state_dir = config.state_dir(&config.read_options()?)?;
            skins::create(&state_dir, &request).map(Json)
        })
        .await;
        eprintln!(
            "msime-mcp: create_candidate_skin {}",
            match &result {
                Ok(created) if created.0.replaced => "replaced",
                Ok(_) => "installed",
                Err(_) => "refused",
            }
        );
        result
    }

    #[tool(
        name = "get_typing_statistics",
        description = "Read aggregate typing statistics: characters per day, per category and per input scheme, time spent typing and typing speed, the distribution over the hours of the day, and how often each candidate position is chosen. Counts only; no text is ever recorded.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn get_typing_statistics(
        &self,
        Parameters(request): Parameters<StatisticsRequest>,
    ) -> Result<Json<StatisticsView>, String> {
        let config = self.config.clone();
        blocking(move || {
            let state_dir = config.state_dir(&config.read_options()?)?;
            statistics::load(&state_dir, &request).map(Json)
        })
        .await
    }

    #[tool(
        name = "read_diagnostic_log",
        description = "Read the most recent lines of the input method's diagnostic log, the record its hosts keep of focus changes, slow requests, candidate window and dictionary events and failures, to look into a problem the user describes. Filter by text and by local time. Also tells whether the log is on; nothing is recorded while it is off.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn read_diagnostic_log(
        &self,
        Parameters(request): Parameters<LogRequest>,
    ) -> Result<Json<LogView>, String> {
        let config = self.config.clone();
        blocking(move || {
            let state_dir = config.state_dir(&config.read_options()?)?;
            diagnostics::load(&state_dir, &request).map(Json)
        })
        .await
    }

    #[tool(
        name = "set_diagnostic_log",
        description = "Turn the input method's diagnostic log on, so that what goes wrong next is recorded, or off once the problem is understood. Only changes whether the input method writes its log on this computer.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn set_diagnostic_log(
        &self,
        Parameters(request): Parameters<SwitchRequest>,
    ) -> Result<Json<SwitchView>, String> {
        let guard = self.claim_write()?;
        let config = self.config.clone();
        let result = blocking(move || {
            let _guard = guard;
            let state_dir = config.state_dir(&config.read_options()?)?;
            diagnostics::set(&state_dir, &config.options, request.enabled).map(Json)
        })
        .await;
        eprintln!(
            "msime-mcp: set_diagnostic_log {}",
            match (&result, request.enabled) {
                (Err(_), _) => "refused",
                (Ok(_), true) => "on",
                (Ok(_), false) => "off",
            }
        );
        result
    }

    #[tool(
        name = "list_dictionary_words",
        description = "List the words the user added to a typing dictionary, or with include_bundled and a code_prefix every word the dictionary has under that code, with the weights that rank them.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_dictionary_words(
        &self,
        Parameters(request): Parameters<WordListRequest>,
    ) -> Result<Json<WordListPage>, String> {
        let limit = request.limit.unwrap_or(DEFAULT_PAGE);
        if !(1..=MAX_PAGE).contains(&limit) {
            return Err(format!("limit must be between 1 and {MAX_PAGE}"));
        }
        let code_prefix = request.code_prefix.unwrap_or_default();
        let include_bundled = request.include_bundled.unwrap_or(false);
        if include_bundled && code_prefix.trim().is_empty() {
            return Err("include_bundled needs a code_prefix".into());
        }
        let config = self.config.clone();
        blocking(move || {
            let options = DictionaryOptions::from_host_document(config.read_host_options()?)?;
            let page = msime_host_api::dictionary_words(
                &options,
                request.dictionary.into(),
                &code_prefix,
                include_bundled,
                request.offset.unwrap_or(0),
                limit,
            )?;
            Ok(Json(WordListPage {
                words: page.words.into_iter().map(Into::into).collect(),
                has_more: page.has_more,
            }))
        })
        .await
    }

    #[tool(
        name = "edit_dictionary_words",
        description = "Add words to a typing dictionary, give a word another weight, or remove one. A word is found by its exact code and word, so list or look it up first. Edits are applied in order and the first failure stops the rest.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn edit_dictionary_words(
        &self,
        Parameters(request): Parameters<WordEditRequest>,
    ) -> Result<Json<EditOutcome>, String> {
        if request.edits.is_empty() || request.edits.len() > MAX_EDITS {
            return Err(format!("send between 1 and {MAX_EDITS} edits"));
        }
        let guard = self.claim_write()?;
        let config = self.config.clone();
        let edits: Vec<WordEdit> = request.edits.into_iter().map(Into::into).collect();
        let outcome = blocking(move || {
            let _guard = guard;
            let options = DictionaryOptions::from_host_document(config.read_host_options()?)?;
            Ok(apply_edits(
                &options,
                &edits,
                msime_host_api::edit_dictionary_word,
            ))
        })
        .await?;
        eprintln!(
            "msime-mcp: edit_dictionary_words applied {}{}",
            outcome.applied,
            if outcome.error.is_some() {
                " then failed"
            } else {
                ""
            }
        );
        Ok(Json(outcome))
    }

    #[tool(
        name = "import_dictionary_words",
        description = "Add up to 200 words to one typing dictionary. Words already there are left as they are and counted; words the dictionary refuses are named by index and the rest still go in, so sending the same words again is safe.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn import_dictionary_words(
        &self,
        Parameters(request): Parameters<WordImportRequest>,
    ) -> Result<Json<WordImportOutcome>, String> {
        if request.words.is_empty() || request.words.len() > words::MAX_IMPORT {
            return Err(format!("send between 1 and {} words", words::MAX_IMPORT));
        }
        let guard = self.claim_write()?;
        let config = self.config.clone();
        let kind = request.dictionary.into();
        let new_words = request.new_words();
        let result = blocking(move || {
            let _guard = guard;
            let options = DictionaryOptions::from_host_document(config.read_host_options()?)?;
            let request_id = msime_client_core::uuid::Uuid::new_v4().simple().to_string();
            let mut hosts = QuiescedHosts::new(Some(options.user_data()), || {});
            hosts.run(|| {
                msime_host_api::import_dictionary_words(&options, kind, &new_words, &request_id)
            })
        })
        .await;
        match &result {
            Ok(outcome) => eprintln!(
                "msime-mcp: import_dictionary_words added {}, found {}, refused {}",
                outcome.added,
                outcome.existing,
                outcome.rejected.len()
            ),
            Err(_) => eprintln!("msime-mcp: import_dictionary_words failed"),
        }
        result.map(|outcome| Json(outcome.into()))
    }

    #[tool(
        name = "lookup_candidates",
        description = "Show the candidates typing a code offers from the local dictionaries, in the input method's order, with where each came from and the weight of each dictionary word. Explains why a word ranks where it does. Types into a session of its own that learns nothing; no cloud or AI suggestions are included.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn lookup_candidates(
        &self,
        Parameters(request): Parameters<LookupRequest>,
    ) -> Result<Json<LookupView>, String> {
        let limit = request.limit.unwrap_or(words::DEFAULT_LOOKUP);
        let config = self.config.clone();
        blocking(move || {
            let options = DictionaryOptions::from_host_document(config.read_host_options()?)?;
            let candidates = msime_host_api::lookup_candidates(
                &options,
                request.scheme.map(Into::into),
                &request.code,
                limit,
            )?;
            Ok(Json(LookupView {
                candidates: candidates.into_iter().map(Into::into).collect(),
            }))
        })
        .await
    }
}

#[tool_handler(router = self.tool_router)]
#[prompt_handler(router = self.prompt_router)]
impl ServerHandler for MsimeServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new("msime", env!("CARGO_PKG_VERSION")))
        .with_instructions(INSTRUCTIONS)
    }
}

impl MsimeServer {
    /// Take the next write slot, or refuse when another write is still running or the previous one was too recent. The guard goes into the blocking work, not the handler future: a cancelled request drops the future while the work runs on.
    fn claim_write(&self) -> Result<WriteGuard, String> {
        if self
            .writing
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            return Err("another change is still being applied; try again when it finishes".into());
        }
        let guard = WriteGuard(self.writing.clone());
        let mut last = self
            .last_write
            .lock()
            .map_err(|_| "the server is shutting down")?;
        let now = Instant::now();
        if last.is_some_and(|previous| now.duration_since(previous) < WRITE_INTERVAL) {
            return Err("writes are limited to one a second; try again shortly".into());
        }
        *last = Some(now);
        Ok(guard)
    }
}

/// Clears the write-in-progress flag when the write it covers ends, however it ends.
struct WriteGuard(Arc<AtomicBool>);

impl Drop for WriteGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Apply `edits` in order under one release, so the input hosts let go of the dictionary once for the whole call rather than once per edit, and are let back when this returns. On Linux and macOS that is a lease the hosts find on their timers; the macOS input method finds it within a second, and the desktop app also tells it at once, which would take the macOS host crate here for a second's difference. On Windows the Server is asked over its pipe and answers once its sessions are gone.
fn apply_edits<E>(
    options: &DictionaryOptions,
    edits: &[E],
    apply: impl Fn(&DictionaryOptions, &E, &str) -> Result<(), String>,
) -> EditOutcome {
    let mut hosts = QuiescedHosts::new(Some(options.user_data()), || {});
    for (index, edit) in edits.iter().enumerate() {
        let request_id = msime_client_core::uuid::Uuid::new_v4().simple().to_string();
        if let Err(error) = hosts.run(|| apply(options, edit, &request_id)) {
            return EditOutcome {
                applied: index,
                failed_index: Some(index),
                error: Some(error),
            };
        }
    }
    EditOutcome {
        applied: edits.len(),
        failed_index: None,
        error: None,
    }
}

/// Run file and dictionary work off the protocol task, so a slow disk or a busy dictionary does not stall the stdio loop.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| "the request was interrupted".to_owned())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn server(allow_write: bool) -> MsimeServer {
        server_with(allow_write, false)
    }

    fn server_with(allow_write: bool, allow_dictionary_read: bool) -> MsimeServer {
        MsimeServer::new(Config {
            options: PathBuf::from("/nonexistent/runtime-options.json"),
            state_dir: None,
            allow_write,
            allow_dictionary_read,
        })
    }

    #[test]
    fn write_tools_are_offered_only_when_allowed() {
        let names = |server: &MsimeServer| {
            let mut names: Vec<String> = server
                .tool_router
                .list_all()
                .into_iter()
                .map(|tool| tool.name.to_string())
                .collect();
            names.sort();
            names
        };
        assert_eq!(
            names(&server(false)),
            [
                "get_preferences",
                "get_typing_statistics",
                "list_candidate_skins",
                "list_quick_phrases",
                "read_diagnostic_log",
                "set_diagnostic_log"
            ]
        );
        assert_eq!(
            names(&server(true)),
            [
                "create_candidate_skin",
                "edit_quick_phrases",
                "get_preferences",
                "get_typing_statistics",
                "list_candidate_skins",
                "list_quick_phrases",
                "read_diagnostic_log",
                "set_diagnostic_log",
                "update_preferences"
            ]
        );
        assert_eq!(
            names(&server_with(false, true)),
            [
                "get_preferences",
                "get_typing_statistics",
                "list_candidate_skins",
                "list_dictionary_words",
                "list_quick_phrases",
                "lookup_candidates",
                "read_diagnostic_log",
                "set_diagnostic_log"
            ]
        );
        assert_eq!(
            names(&server_with(true, true)),
            [
                "create_candidate_skin",
                "edit_dictionary_words",
                "edit_quick_phrases",
                "get_preferences",
                "get_typing_statistics",
                "import_dictionary_words",
                "list_candidate_skins",
                "list_dictionary_words",
                "list_quick_phrases",
                "lookup_candidates",
                "read_diagnostic_log",
                "set_diagnostic_log",
                "update_preferences"
            ]
        );
    }

    #[test]
    fn writes_are_spaced_out() {
        let server = server(true);
        drop(server.claim_write().unwrap());
        assert!(server.claim_write().is_err());
        *server.last_write.lock().unwrap() = Some(Instant::now() - WRITE_INTERVAL);
        server.claim_write().unwrap();
    }

    #[test]
    fn a_running_write_holds_off_the_next() {
        let server = server(true);
        let guard = server.claim_write().unwrap();
        *server.last_write.lock().unwrap() = Some(Instant::now() - WRITE_INTERVAL);
        assert!(server.claim_write().is_err());
        // The refusal did not take the slot.
        drop(guard);
        server.claim_write().unwrap();
    }

    #[tokio::test]
    async fn an_empty_preferences_change_does_not_take_the_slot() {
        let server = server(true);
        let change: PreferencesChange =
            serde_json::from_value(serde_json::json!({ "expected_revision": 0 })).unwrap();
        assert_eq!(
            server
                .update_preferences(Parameters(change))
                .await
                .err()
                .as_deref(),
            Some("no preference to change")
        );
        server.claim_write().unwrap();
    }

    #[test]
    fn edits_read_as_the_engine_edits() {
        let request: EditRequest = serde_json::from_value(serde_json::json!({ "edits": [
            { "op": "add", "code": "yx", "text": "邮箱" },
            { "op": "replace", "previous": { "code": "yx", "text": "邮箱" }, "replacement": { "code": "yx", "text": "someone@example.com" } },
            { "op": "remove", "code": "yx", "text": "someone@example.com" }
        ]}))
        .unwrap();
        let edits: Vec<QuickPhraseEdit> = request.edits.into_iter().map(Into::into).collect();
        let phrase = |text: &str| QuickPhrase {
            code: "yx".into(),
            text: text.into(),
        };
        assert_eq!(
            edits,
            [
                QuickPhraseEdit::Add(phrase("邮箱")),
                QuickPhraseEdit::Replace {
                    previous: phrase("邮箱"),
                    replacement: phrase("someone@example.com")
                },
                QuickPhraseEdit::Remove(phrase("someone@example.com")),
            ]
        );
        assert!(serde_json::from_value::<EditRequest>(
            serde_json::json!({ "edits": [{ "op": "add", "code": "a", "text": "b", "weight": 1 }] })
        )
        .is_err());
    }
}
