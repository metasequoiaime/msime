//! The device's anonymous MSIME account, registered when the client is first installed or run so services that need an account work before anyone signs in.
//!
//! The identity is a client-generated subject and secret, the same shape the macOS/iOS (`shared/backend/account/BackendAnonymousAccount.swift`), Android (`BackendAnonymousAccount.java`) and Linux (`msime-linux-online-provider`) clients create: `msime-` plus 16 characters, and a 48-character secret, both drawn from `[a-z0-9]`. Whoever presents the subject first creates the account. The files are those clients' names and layout too: `anonymous-account.json` holds `{subject, secret}` and `anonymous-session.json` holds a `SavedAccountSession`.

use super::*;
use std::path::PathBuf;

pub const ANONYMOUS_ACCOUNT_FILE: &str = "anonymous-account.json";
pub const ANONYMOUS_SESSION_FILE: &str = "anonymous-session.json";
const SUBJECT_PREFIX: &str = "msime-";
const SUBJECT_LENGTH: usize = 16;
const SECRET_LENGTH: usize = 48;
const ALPHABET: &[u8; 36] = b"abcdefghijklmnopqrstuvwxyz0123456789";
const MAX_FILE_BYTES: u64 = 64 * 1024;

#[derive(Clone, Serialize, Deserialize)]
pub struct AnonymousIdentity {
    pub subject: String,
    pub secret: String,
}

impl AnonymousIdentity {
    fn generate() -> Self {
        Self {
            subject: format!("{SUBJECT_PREFIX}{}", random_text(SUBJECT_LENGTH)),
            secret: random_text(SECRET_LENGTH),
        }
    }

    fn is_valid(&self) -> bool {
        self.subject
            .strip_prefix(SUBJECT_PREFIX)
            .is_some_and(|suffix| valid_anonymous_text(suffix, SUBJECT_LENGTH))
            && valid_anonymous_text(&self.secret, SECRET_LENGTH)
    }
}

pub(super) fn valid_anonymous_subject(value: &str) -> bool {
    value
        .strip_prefix(SUBJECT_PREFIX)
        .is_some_and(|suffix| valid_anonymous_text(suffix, SUBJECT_LENGTH))
}

fn valid_anonymous_text(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| ALPHABET.contains(&byte))
}

/// Uniform characters from the OS random source. A v4 UUID is 122 bits from `getrandom`, which is the CSPRNG every supported platform provides; bytes past the largest multiple of 36 are rejected so no character is favoured.
fn random_text(length: usize) -> String {
    let mut text = String::with_capacity(length);
    while text.len() < length {
        for byte in uuid::Uuid::new_v4().into_bytes() {
            if byte < 252 && text.len() < length {
                text.push(char::from(ALPHABET[usize::from(byte % 36)]));
            }
        }
    }
    text
}

/// `anonymous-session.json` in the host's directory, written the way the other clients write it: replaced whole, readable only by the owner.
pub struct AnonymousSessionStorage {
    directory: PathBuf,
}

impl AnonymousSessionStorage {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }
}

impl AccountSessionStorage for AnonymousSessionStorage {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        read_private_json(&self.directory.join(ANONYMOUS_SESSION_FILE))
    }

    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
        let bytes = serde_json::to_vec(session).map_err(|_| AccountError::Storage)?;
        prepare_directory(&self.directory)?;
        write_private(&self.directory, ANONYMOUS_SESSION_FILE, &bytes, false).map(|_| ())
    }

    fn clear(&self) -> Result<(), AccountError> {
        crate::storage::reject_symlink(&self.directory).map_err(|_| AccountError::Storage)?;
        match crate::storage::remove_private_file(&self.directory.join(ANONYMOUS_SESSION_FILE)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(AccountError::Storage),
        }
    }
}

/// Registers the device's anonymous account under `directory` unless it already has a session there. Blocks on the network: call it off the thread that handles input. Safe to call on every start; once registered it only reads the session file.
pub fn ensure_anonymous_account(directory: &Path) -> Result<(), AccountError> {
    // Checked before building the HTTP client, so a start after registration costs one file read.
    if has_session(directory) {
        return Ok(());
    }
    ensure_anonymous_account_with(BackendAccountClient::new()?, directory)
}

pub(super) fn ensure_anonymous_account_with<A: AccountApi>(
    api: A,
    directory: &Path,
) -> Result<(), AccountError> {
    prepare_directory(directory)?;
    if has_session(directory) {
        return Ok(());
    }
    let identity = anonymous_identity(directory)?;
    BackendAccountSession::new(api, AnonymousSessionStorage::new(directory))
        .sign_in_anonymous(&identity.subject, &identity.secret)
        .map(|_| ())
}

/// Whether a readable session is saved. One that cannot be read is replaced by signing in again with the saved identity, which loses nothing: the session is only tokens the backend reissues for that identity.
fn has_session(directory: &Path) -> bool {
    matches!(AnonymousSessionStorage::new(directory).load(), Ok(Some(_)))
}

/// The saved identity, or a new one. A new identity is published with no-clobber, so two processes starting together (HarmonyOS runs the app and the keyboard separately) end up with the one that landed first rather than registering two accounts.
fn anonymous_identity(directory: &Path) -> Result<AnonymousIdentity, AccountError> {
    let path = directory.join(ANONYMOUS_ACCOUNT_FILE);
    if let Some(saved) = read_private_json::<AnonymousIdentity>(&path)? {
        return saved
            .is_valid()
            .then_some(saved)
            .ok_or(AccountError::Storage);
    }
    let identity = AnonymousIdentity::generate();
    let bytes = serde_json::to_vec(&identity).map_err(|_| AccountError::Storage)?;
    if write_private(directory, ANONYMOUS_ACCOUNT_FILE, &bytes, true)? {
        return Ok(identity);
    }
    read_private_json::<AnonymousIdentity>(&path)?
        .filter(AnonymousIdentity::is_valid)
        .ok_or(AccountError::Storage)
}

fn prepare_directory(directory: &Path) -> Result<(), AccountError> {
    crate::storage::create_directory_and_check(directory).map_err(|_| AccountError::Storage)?;
    let metadata = std::fs::symlink_metadata(directory).map_err(|_| AccountError::Storage)?;
    if !metadata.file_type().is_dir() {
        return Err(AccountError::Storage);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))
                .map_err(|_| AccountError::Storage)?;
        }
    }
    Ok(())
}

fn read_private_file(file: std::fs::File) -> Result<Vec<u8>, AccountError> {
    crate::bounded_io::read_bounded_file_with(
        file,
        MAX_FILE_BYTES,
        || AccountError::Storage,
        |_| AccountError::Storage,
    )
}

fn read_private_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>, AccountError> {
    if let Some(parent) = path.parent() {
        crate::storage::reject_symlink(parent).map_err(|_| AccountError::Storage)?;
    }
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(AccountError::Storage),
    };
    if !metadata.file_type().is_file() {
        return Err(AccountError::Storage);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let Some(parent) = path.parent() else {
            return Err(AccountError::Storage);
        };
        let directory = std::fs::symlink_metadata(parent).map_err(|_| AccountError::Storage)?;
        if metadata.mode() & 0o077 != 0 || metadata.uid() != directory.uid() {
            return Err(AccountError::Storage);
        }
    }
    // Bound the read through the handle so a concurrent replacement cannot bypass the size limit.
    let bytes = read_private_file(
        crate::storage::open_private_file(path).map_err(|_| AccountError::Storage)?,
    )?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| AccountError::Storage)
}

/// Stages `bytes` beside the target and renames it into place. `NamedTempFile` is created owner-only on Unix; on Windows and HarmonyOS the directory is already private to the user or the app. With `no_clobber`, returns `false` instead of replacing a file another process published first.
fn write_private(
    directory: &Path,
    name: &str,
    bytes: &[u8],
    no_clobber: bool,
) -> Result<bool, AccountError> {
    prepare_directory(directory)?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(directory).map_err(|_| AccountError::Storage)?;
    temporary
        .write_all(bytes)
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|_| AccountError::Storage)?;
    let path = directory.join(name);
    if no_clobber {
        return match temporary.persist_noclobber(&path) {
            Ok(_) => Ok(true),
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
            Err(_) => Err(AccountError::Storage),
        };
    }
    temporary
        .persist(&path)
        .map(|_| true)
        .map_err(|_| AccountError::Storage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone, Default)]
    struct RegisteringApi {
        challenges: Arc<Mutex<Vec<(String, String)>>>,
        logins: Arc<Mutex<Vec<(String, String)>>>,
        failures: Arc<AtomicUsize>,
    }

    impl AccountApi for RegisteringApi {
        fn challenge(
            &self,
            provider: &str,
            target: &str,
        ) -> Result<AccountChallenge, AccountError> {
            validate_provider_target(provider, target)?;
            if self.failures.load(Ordering::SeqCst) > 0 {
                self.failures.fetch_sub(1, Ordering::SeqCst);
                return Err(AccountError::Unavailable);
            }
            self.challenges
                .lock()
                .unwrap()
                .push((provider.into(), target.into()));
            Ok(AccountChallenge {
                challenge_id: "fixture-challenge".into(),
                expires_in: 300,
                nonce: None,
                authorization_url: None,
            })
        }

        fn login(&self, challenge: &str, credential: &str) -> Result<AccountTokens, AccountError> {
            validate_login_request(challenge, credential)?;
            self.logins
                .lock()
                .unwrap()
                .push((challenge.into(), credential.into()));
            Ok(AccountTokens {
                access_token: "a".repeat(64),
                refresh_token: "b".repeat(64),
                token_type: "Bearer".into(),
                expires_in: 3600,
                user: AccountUser {
                    id: "fixture-anonymous".into(),
                    display_name: String::new(),
                    created_at: "2026-01-01T00:00:00Z".into(),
                    email: None,
                    avatar_url: None,
                },
            })
        }

        fn providers(&self) -> Result<std::collections::HashMap<String, bool>, AccountError> {
            Ok(Default::default())
        }

        fn refresh(&self, _refresh_token: &str) -> Result<AccountTokens, AccountError> {
            Err(AccountError::Unavailable)
        }

        fn profile(&self, _access_token: &str) -> Result<AccountProfile, AccountError> {
            Err(AccountError::Unavailable)
        }

        fn rename(&self, _display_name: &str, _access_token: &str) -> Result<(), AccountError> {
            Err(AccountError::Unavailable)
        }

        fn logout(&self, _access_token: &str, _all: bool) -> Result<(), AccountError> {
            Err(AccountError::Unavailable)
        }

        fn delete_account(&self, _access_token: &str) -> Result<(), AccountError> {
            Err(AccountError::Unavailable)
        }
    }

    #[test]
    fn registers_once_with_the_shared_identity_shape() {
        let dir = tempfile::tempdir().unwrap();
        let api = RegisteringApi::default();
        ensure_anonymous_account_with(api.clone(), dir.path()).unwrap();
        ensure_anonymous_account_with(api.clone(), dir.path()).unwrap();

        let identity: AnonymousIdentity = serde_json::from_slice(
            &std::fs::read(dir.path().join(ANONYMOUS_ACCOUNT_FILE)).unwrap(),
        )
        .unwrap();
        assert!(identity.is_valid());
        assert_eq!(
            *api.challenges.lock().unwrap(),
            [("anonymous".to_string(), identity.subject.clone())]
        );
        assert_eq!(
            *api.logins.lock().unwrap(),
            [("fixture-challenge".to_string(), identity.secret.clone())]
        );
        let saved = AnonymousSessionStorage::new(dir.path())
            .load()
            .unwrap()
            .unwrap();
        assert_eq!(saved.tokens.access_token, "a".repeat(64));
        #[cfg(unix)]
        for name in [ANONYMOUS_ACCOUNT_FILE, ANONYMOUS_SESSION_FILE] {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join(name))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o077, 0, "{name} is readable by others");
        }
    }

    #[test]
    fn a_failed_registration_keeps_the_identity_for_the_retry() {
        let dir = tempfile::tempdir().unwrap();
        let api = RegisteringApi::default();
        api.failures.store(1, Ordering::SeqCst);
        assert_eq!(
            ensure_anonymous_account_with(api.clone(), dir.path()).err(),
            Some(AccountError::Unavailable)
        );
        let first = std::fs::read(dir.path().join(ANONYMOUS_ACCOUNT_FILE)).unwrap();
        ensure_anonymous_account_with(api.clone(), dir.path()).unwrap();
        assert_eq!(
            std::fs::read(dir.path().join(ANONYMOUS_ACCOUNT_FILE)).unwrap(),
            first
        );
        assert_eq!(api.challenges.lock().unwrap().len(), 1);
    }

    #[test]
    fn an_identity_published_first_by_another_process_wins() {
        let dir = tempfile::tempdir().unwrap();
        let theirs = AnonymousIdentity::generate();
        let bytes = serde_json::to_vec(&theirs).unwrap();
        assert!(write_private(dir.path(), ANONYMOUS_ACCOUNT_FILE, &bytes, true).unwrap());
        assert!(!write_private(dir.path(), ANONYMOUS_ACCOUNT_FILE, b"{}", true).unwrap());
        assert_eq!(
            anonymous_identity(dir.path()).unwrap().subject,
            theirs.subject
        );
    }

    #[test]
    fn an_unreadable_session_is_replaced_by_signing_in_again() {
        let dir = tempfile::tempdir().unwrap();
        let api = RegisteringApi::default();
        ensure_anonymous_account_with(api.clone(), dir.path()).unwrap();
        std::fs::write(dir.path().join(ANONYMOUS_SESSION_FILE), b"not json").unwrap();
        ensure_anonymous_account_with(api.clone(), dir.path()).unwrap();
        assert_eq!(api.logins.lock().unwrap().len(), 2);
        assert!(AnonymousSessionStorage::new(dir.path())
            .load()
            .unwrap()
            .is_some());
    }

    #[test]
    fn a_corrupt_identity_is_not_replaced() {
        // Replacing it would register a second account and orphan whatever the first one owns.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(ANONYMOUS_ACCOUNT_FILE),
            br#"{"subject":"msime-short","secret":"x"}"#,
        )
        .unwrap();
        assert_eq!(
            ensure_anonymous_account_with(RegisteringApi::default(), dir.path()).err(),
            Some(AccountError::Storage)
        );
    }

    #[test]
    fn generated_text_uses_only_the_shared_alphabet() {
        for _ in 0..64 {
            let identity = AnonymousIdentity::generate();
            assert!(identity.is_valid());
            assert!(valid_anonymous_subject(&identity.subject));
        }
        assert!(!valid_anonymous_subject("msime-ABCDEFGHIJKLMNOP"));
        assert!(!valid_anonymous_subject("someone@example.com"));
    }

    #[test]
    fn anonymous_json_file_read_reserves_file_size() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("identity.json");
        let contents = br#"{"subject":"msime-synthetic","secret":"synthetic"}"#;
        std::fs::write(&path, contents).unwrap();

        let bytes = read_private_file(std::fs::File::open(path).unwrap()).unwrap();
        assert_eq!(bytes, contents);
        assert_eq!(bytes.capacity(), contents.len());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlinked_anonymous_directory() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let linked = root.path().join("anonymous");
        symlink(outside.path(), &linked).unwrap();

        assert_eq!(
            ensure_anonymous_account_with(RegisteringApi::default(), &linked).err(),
            Some(AccountError::Storage)
        );
        assert!(!outside.path().join(ANONYMOUS_ACCOUNT_FILE).exists());
        assert!(!outside.path().join(ANONYMOUS_SESSION_FILE).exists());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlinked_anonymous_session_file() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = outside.path().join("session.json");
        std::fs::write(&target, br#"{}"#).unwrap();
        let linked = root.path().join(ANONYMOUS_SESSION_FILE);
        symlink(&target, &linked).unwrap();

        assert_eq!(
            AnonymousSessionStorage::new(root.path()).load().err(),
            Some(AccountError::Storage)
        );
        assert_eq!(std::fs::read(&target).unwrap(), br#"{}"#);
    }
}
