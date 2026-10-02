//! Notices published in the admin console, read from the public `GET /v1/notices` feed.
//!
//! The feed is the same for everyone and the server marks it `Cache-Control: public, max-age=60`, so a host fetches it when the settings window or app home opens and never more often than once a minute ([`MIN_REFRESH_INTERVAL`]); the input-method process does not poll it in the background. [`NoticeStore`] keeps the last feed and the ids the user dismissed, so a dismissed notice stays dismissed on this device.
//!
//! Bodies are simple Markdown. [`markdown_to_html`] renders one to HTML that is safe to hand to a rich-text view: raw HTML in the source is escaped, only `http`, `https` and `mailto` links survive, and images become links. The React surfaces render with markdown-it and Apple, Android and the Windows WebView2 page with their own renderers; this one is for the hosts without a renderer of their own (HarmonyOS RichText).

use crate::account::{AccountError, BackendAccountClient};
use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The cache of the last feed and the dismissed ids.
pub const NOTICES_FILE: &str = "notices.json";
/// The server's `max-age`; the feed is not requested again sooner, whether the last attempt worked or not.
pub const MIN_REFRESH_INTERVAL: Duration = Duration::from_secs(60);
/// Most notices the server lists.
pub const MAX_NOTICES: usize = 20;
const MAX_TITLE_CHARS: usize = 200;
const MAX_BODY_CHARS: usize = 20_000;
const MAX_LIST_ENTRIES: usize = 16;
const MAX_LIST_ENTRY_CHARS: usize = 32;
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_CACHE_BYTES: u64 = 4 * 1024 * 1024;
/// Dismissed ids kept; the oldest are forgotten first. Ids no longer in the feed are forgotten when the feed is refreshed.
const MAX_DISMISSED: usize = 200;

/// Which audience a feed is for: the apps' settings windows, or the website.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoticeChannel {
    App,
    Site,
}

impl NoticeChannel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Site => "site",
        }
    }
}

/// One live notice, newest first in a feed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notice {
    pub id: String,
    pub title: String,
    /// Simple Markdown; see [`markdown_to_html`].
    pub body: String,
    #[serde(default)]
    pub targets: Vec<String>,
    #[serde(default)]
    pub channels: Vec<String>,
    /// RFC 3339.
    pub published_at: String,
}

impl Notice {
    fn is_valid(&self) -> bool {
        let list = |values: &[String]| {
            values.len() <= MAX_LIST_ENTRIES
                && values.iter().all(|value| {
                    crate::community::valid_text(value, 1, MAX_LIST_ENTRY_CHARS, false)
                })
        };
        (1..=64).contains(&self.id.len())
            && self
                .id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
            && crate::community::valid_text(&self.title, 1, MAX_TITLE_CHARS, false)
            && crate::community::valid_text(&self.body, 0, MAX_BODY_CHARS, true)
            && list(&self.targets)
            && list(&self.channels)
            && crate::community::valid_text(&self.published_at, 1, 64, false)
    }
}

#[derive(Deserialize)]
struct NoticeFeed {
    items: Vec<Notice>,
}

/// Fetches the live notices for `channel` on `platform` (a canonical platform id or one of its aliases). Unauthenticated; an item that breaks the feed's limits is left out rather than failing the rest.
pub fn fetch_notices(
    client: &BackendAccountClient,
    channel: NoticeChannel,
    platform: &str,
) -> Result<Vec<Notice>, AccountError> {
    let platform = crate::telemetry::canonical_platform(platform).ok_or(AccountError::Invalid)?;
    let feed = client.json_with_limit::<NoticeFeed, ()>(
        Method::GET,
        &format!(
            "/v1/notices?channel={}&platform={platform}",
            channel.as_str()
        ),
        None,
        None,
        MAX_RESPONSE_BYTES,
    )?;
    Ok(valid_items(feed.items))
}

fn valid_items(items: Vec<Notice>) -> Vec<Notice> {
    items
        .into_iter()
        .filter(Notice::is_valid)
        .take(MAX_NOTICES)
        .collect()
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum NoticeError {
    #[error("invalid notice input")]
    Invalid,
    #[error("notice storage is unavailable")]
    Storage,
}

impl From<std::io::Error> for NoticeError {
    fn from(_: std::io::Error) -> Self {
        Self::Storage
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct NoticeCache {
    /// `<channel>/<platform>` the items were fetched for.
    #[serde(default)]
    feed: String,
    #[serde(default)]
    attempted_at_unix_ms: u64,
    #[serde(default)]
    items: Vec<Notice>,
    #[serde(default)]
    dismissed: Vec<String>,
}

/// The cached feed and dismissed ids of one host, in a directory it owns.
#[derive(Clone, Debug)]
pub struct NoticeStore {
    directory: PathBuf,
}

impl NoticeStore {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }

    /// The notices to show: the cached feed when it was requested less than [`MIN_REFRESH_INTERVAL`] ago, otherwise a fresh one from `fetch` (the cached one when that fails), without the ones the user dismissed.
    pub fn current(
        &self,
        channel: NoticeChannel,
        platform: &str,
        now: SystemTime,
        fetch: impl FnOnce() -> Result<Vec<Notice>, AccountError>,
    ) -> Result<Vec<Notice>, NoticeError> {
        let platform =
            crate::telemetry::canonical_platform(platform).ok_or(NoticeError::Invalid)?;
        let feed = format!("{}/{platform}", channel.as_str());
        let mut cache = self.read();
        if cache.feed != feed {
            cache.items.clear();
            cache.attempted_at_unix_ms = 0;
        }
        let now_ms = unix_ms(now);
        let interval = u64::try_from(MIN_REFRESH_INTERVAL.as_millis()).unwrap_or(u64::MAX);
        let fresh = cache.feed == feed
            && now_ms >= cache.attempted_at_unix_ms
            && now_ms - cache.attempted_at_unix_ms < interval;
        if !fresh {
            cache.feed = feed;
            cache.attempted_at_unix_ms = now_ms;
            if let Ok(items) = fetch() {
                cache.items = valid_items(items);
                cache
                    .dismissed
                    .retain(|id| cache.items.iter().any(|item| &item.id == id));
            }
            self.write(&cache)?;
        }
        Ok(cache
            .items
            .into_iter()
            .filter(|item| !cache.dismissed.contains(&item.id))
            .collect())
    }

    /// Remembers that the user dismissed notice `id`.
    pub fn dismiss(&self, id: &str) -> Result<(), NoticeError> {
        if !(1..=64).contains(&id.len())
            || !id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err(NoticeError::Invalid);
        }
        let mut cache = self.read();
        if cache.dismissed.iter().any(|dismissed| dismissed == id) {
            return Ok(());
        }
        cache.dismissed.push(id.to_owned());
        if cache.dismissed.len() > MAX_DISMISSED {
            let excess = cache.dismissed.len() - MAX_DISMISSED;
            cache.dismissed.drain(..excess);
        }
        self.write(&cache)
    }

    fn read(&self) -> NoticeCache {
        let path = self.directory.join(NOTICES_FILE);
        if !self.directory.is_absolute() || crate::storage::reject_symlink(&path).is_err() {
            return NoticeCache::default();
        }
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            return NoticeCache::default();
        };
        if !metadata.file_type().is_file() {
            return NoticeCache::default();
        }
        File::open(path)
            .ok()
            .and_then(|file| crate::bounded_io::read_bounded(file, MAX_CACHE_BYTES).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn write(&self, cache: &NoticeCache) -> Result<(), NoticeError> {
        if !self.directory.is_absolute()
            || !crate::storage::create_directory_and_check(&self.directory)?
        {
            return Err(NoticeError::Storage);
        }
        let bytes = serde_json::to_vec(cache).map_err(|_| NoticeError::Storage)?;
        let mut temporary = tempfile::NamedTempFile::new_in(&self.directory)?;
        temporary.write_all(&bytes)?;
        temporary
            .persist(self.directory.join(NOTICES_FILE))
            .map_err(|_| NoticeError::Storage)?;
        Ok(())
    }
}

fn unix_ms(now: SystemTime) -> u64 {
    now.duration_since(UNIX_EPOCH).map_or(0, |duration| {
        u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
    })
}

/// Renders a notice body to HTML for a rich-text view. Raw HTML in the source is escaped and shown as text; links keep only `http`, `https` and `mailto` targets (any other link keeps its text and loses the link); images are never loaded, a safe one becomes a link with its alt text. Opening links externally is the host's job.
pub fn markdown_to_html(markdown: &str) -> String {
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES;
    // For each open link or image, whether its start tag was kept, so the matching end tag is kept or dropped with it.
    let mut open: Vec<bool> = Vec::new();
    let events = Parser::new_ext(markdown, options).filter_map(|event| match event {
        Event::Html(text) | Event::InlineHtml(text) => Some(Event::Text(text)),
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            let safe = safe_url(&dest_url);
            open.push(safe);
            safe.then_some(Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }))
        }
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            let safe = safe_url(&dest_url);
            open.push(safe);
            safe.then_some(Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }))
        }
        Event::End(TagEnd::Link | TagEnd::Image) => open
            .pop()
            .unwrap_or(false)
            .then_some(Event::End(TagEnd::Link)),
        other => Some(other),
    });
    let mut html = String::with_capacity(markdown.len() * 3 / 2);
    pulldown_cmark::html::push_html(&mut html, events);
    html
}

fn safe_url(url: &CowStr<'_>) -> bool {
    let url = url.trim();
    let Some((scheme, rest)) = url.split_once(':') else {
        return false;
    };
    let scheme = scheme.to_ascii_lowercase();
    match scheme.as_str() {
        "http" | "https" => rest.starts_with("//") && rest.len() > 2,
        "mailto" => !rest.is_empty(),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
