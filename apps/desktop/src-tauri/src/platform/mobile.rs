//! Host integration shared by iOS and Android.

#[cfg(any(target_os = "ios", target_os = "android"))]
pub(crate) mod mobile_account_helpers;
// The account preference mapping is plain data, so host tests compile it to exercise the iOS upload and apply rules.
pub(crate) mod mobile_account_preferences;
#[cfg(any(target_os = "ios", target_os = "android"))]
pub(crate) mod mobile_community;

#[cfg(any(target_os = "ios", target_os = "android"))]
use msime_client_core::account::{BackendAccountClient, BackendAccountSession};

/// The account session storage of the running mobile target. Shared commands name it instead of either platform's type, so one body compiles for both.
#[cfg(target_os = "android")]
pub(crate) type MobileStorage = super::android::android_account::AndroidAccountStorage<tauri::Wry>;
#[cfg(target_os = "ios")]
pub(crate) type MobileStorage = super::ios::ios_account::IosAccountStorage<tauri::Wry>;

/// The account session of the running mobile target, the same type each platform's account state holds.
#[cfg(any(target_os = "ios", target_os = "android"))]
pub(crate) type MobileSession = BackendAccountSession<BackendAccountClient, MobileStorage>;

/// The account state the running mobile target manages. Each platform exposes its session through `AccountState::session`, which is all the shared account-backed commands read.
#[cfg(target_os = "android")]
pub(crate) type MobileAccountState = super::android::android_account::AccountState;
#[cfg(target_os = "ios")]
pub(crate) type MobileAccountState = super::ios::ios_account::AccountState;
