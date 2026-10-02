//! Account avatars: reading an image the user picked for upload, and fetching the avatar a user
//! object names so a host can hand it to a page whose content security policy only loads images
//! from `data:` URLs.

use super::*;

/// The largest image the backend accepts for upload; it re-encodes whatever it takes as a 256×256 JPEG.
pub const MAX_ACCOUNT_AVATAR_UPLOAD_BYTES: u64 = 1024 * 1024;
/// The largest avatar fetched for display. Stored avatars are a few tens of KiB; this only stops a host from reading something that is not one.
const MAX_ACCOUNT_AVATAR_FETCH_BYTES: u64 = 2 * 1024 * 1024;
/// Where uploaded avatars are served from: the public bucket behind the backend.
const UPLOADED_AVATAR_HOST: &str = "media.msime.app";
/// Google's profile picture host, and every subdomain of it (lh3., lh4. and so on).
const GOOGLE_AVATAR_HOST: &str = "googleusercontent.com";

/// An avatar image and the type its bytes were sniffed as.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountAvatarImage {
    pub content_type: &'static str,
    pub bytes: Vec<u8>,
}

impl AccountAvatarImage {
    /// The image as a `data:` URL, which is how a page restricted to `img-src 'self' data:` shows it.
    pub fn data_url(&self) -> String {
        use base64::Engine;
        format!(
            "data:{};base64,{}",
            self.content_type,
            base64::engine::general_purpose::STANDARD.encode(&self.bytes)
        )
    }
}

/// Whether `url` is an avatar this client will fetch: HTTPS, no credentials, on the upload bucket or Google's picture host. The URL comes from the backend, but the client still never follows it anywhere else.
pub fn account_avatar_url_allowed(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    if parsed.scheme() != "https"
        || parsed.username() != ""
        || parsed.password().is_some()
        || parsed.port().is_some()
    {
        return false;
    }
    match parsed.host_str() {
        Some(host) => {
            host == UPLOADED_AVATAR_HOST
                || host == GOOGLE_AVATAR_HOST
                || host.ends_with(&format!(".{GOOGLE_AVATAR_HOST}"))
        }
        None => false,
    }
}

/// Whether `url` names an uploaded avatar rather than the Google picture, which decides whether a page offers to remove it.
pub fn account_avatar_is_uploaded(url: &str) -> bool {
    account_avatar_url_allowed(url)
        && Url::parse(url).is_ok_and(|parsed| parsed.host_str() == Some(UPLOADED_AVATAR_HOST))
}

/// The image type `bytes` start like, whatever any header says.
fn sniff_avatar(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// Downloads an avatar for display. Only an allowed URL is fetched, redirects are not followed, and only a PNG, JPEG or WebP within the size bound is returned.
pub fn fetch_account_avatar(url: &str) -> Result<AccountAvatarImage, AccountError> {
    if !account_avatar_url_allowed(url) {
        return Err(AccountError::Invalid);
    }
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|_| AccountError::Unavailable)?;
    let response = client
        .get(url)
        .header(reqwest::header::ACCEPT, "image/png, image/jpeg, image/webp")
        .send()
        .map_err(|_| AccountError::Unavailable)?;
    let bytes = read_bounded_response(response, MAX_ACCOUNT_AVATAR_FETCH_BYTES as usize)?;
    let content_type = sniff_avatar(&bytes).ok_or(AccountError::Unavailable)?;
    Ok(AccountAvatarImage {
        content_type,
        bytes,
    })
}

/// Reads an image the user picked for upload: a regular file, not reached through a symlink, at most [`MAX_ACCOUNT_AVATAR_UPLOAD_BYTES`], and a PNG or JPEG by its contents. The backend checks all of this again; checking here first turns a wrong file into a message instead of a round trip.
pub fn read_account_avatar_upload(path: &Path) -> Result<AccountAvatarImage, AccountError> {
    if !path.is_absolute() {
        return Err(AccountError::Invalid);
    }
    crate::storage::reject_symlink(path).map_err(|_| AccountError::Invalid)?;
    let metadata = std::fs::symlink_metadata(path).map_err(|_| AccountError::Invalid)?;
    if !metadata.file_type().is_file() {
        return Err(AccountError::Invalid);
    }
    if metadata.len() == 0 || metadata.len() > MAX_ACCOUNT_AVATAR_UPLOAD_BYTES {
        return Err(AccountError::Invalid);
    }
    let file = std::fs::File::open(path).map_err(|_| AccountError::Invalid)?;
    let bytes = crate::bounded_io::read_bounded(file, MAX_ACCOUNT_AVATAR_UPLOAD_BYTES)
        .map_err(|_| AccountError::Invalid)?;
    match sniff_avatar(&bytes) {
        Some(content_type @ ("image/png" | "image/jpeg")) => Ok(AccountAvatarImage {
            content_type,
            bytes,
        }),
        _ => Err(AccountError::Invalid),
    }
}
