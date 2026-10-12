use super::{CommunityDeletionResponse, CommunityRatingRequest, CommunityRatingResponse};
use crate::account::AccountError;
use uuid::Uuid;

/// 三类社区资源沿用同一套评分入参和回包核对规则。
pub(crate) fn rate_community_item(
    base_path: &str,
    id: Uuid,
    stars: u8,
    send: impl FnOnce(&str, &CommunityRatingRequest) -> Result<CommunityRatingResponse, AccountError>,
) -> Result<(), AccountError> {
    if id.is_nil() || !(1..=5).contains(&stars) {
        return Err(AccountError::Invalid);
    }
    let path = format!("{base_path}/{}/rating", id.hyphenated());
    let result = send(&path, &CommunityRatingRequest { stars })?;
    if result.stars != stars {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}

/// 三类社区资源沿用同一套删除入参和回包核对规则。
pub(crate) fn delete_community_item(
    base_path: &str,
    id: Uuid,
    send: impl FnOnce(&str) -> Result<CommunityDeletionResponse, AccountError>,
) -> Result<(), AccountError> {
    if id.is_nil() {
        return Err(AccountError::Invalid);
    }
    let path = format!("{base_path}/{}", id.hyphenated());
    let result = send(&path)?;
    if !result.deleted {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}
