//! Music platforms behind one trait. A new platform = a new module implementing
//! [`MusicProvider`]; commands and the UI never contain provider-specific logic.

use async_trait::async_trait;

use crate::error::AppResult;
use crate::model::{Account, Page, Playlist, ProviderId, Signal, SourceRef, Track};

/// Opaque paging cursor: Spotify offset/`after`, YouTube `pageToken`.
pub type Cursor = Option<String>;

/// Cursor-paged provider result (provider order, before unification).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPage<T> {
    pub items: Vec<T>,
    pub next: Cursor,
    pub total: Option<u32>,
}

impl<T> ProviderPage<T> {
    /// Converts to the UI's offset page, given the offset this page started at.
    pub fn into_page(self, offset: u32) -> Page<T> {
        let count = u32::try_from(self.items.len()).unwrap_or(u32::MAX);
        Page {
            total: self.total.unwrap_or(offset.saturating_add(count)),
            next_offset: self.next.as_ref().map(|_| offset.saturating_add(count)),
            items: self.items,
        }
    }
}

/// One music platform (spec §4, Plan "Provider trait"). Implementations live in
/// `providers/<name>/` and own auth refresh, paging, rate limits and mapping.
#[async_trait]
pub trait MusicProvider: Send + Sync {
    fn id(&self) -> ProviderId;

    /// Runs the interactive login (PKCE loopback) and returns the connected account.
    async fn connect(&self) -> AppResult<Account>;

    /// Current account state, without network calls where possible.
    async fn account(&self) -> AppResult<Account>;

    async fn liked_tracks(&self, cursor: Cursor) -> AppResult<ProviderPage<Track>>;

    async fn playlists(&self, cursor: Cursor) -> AppResult<ProviderPage<Playlist>>;

    async fn playlist_items(
        &self,
        playlist_id: &str,
        cursor: Cursor,
    ) -> AppResult<ProviderPage<Track>>;

    /// Top items, recently played, likes, follows — whatever the platform exposes.
    async fn taste_signals(&self) -> AppResult<Vec<Signal>>;

    /// Catalog search (YouTube: costs quota; only on explicit user request).
    async fn search(&self, query: &str) -> AppResult<Vec<Track>>;

    async fn play(&self, item: &SourceRef) -> AppResult<()>;

    /// Forgets tokens for this provider (local only; does not revoke on the platform).
    async fn disconnect(&self) -> AppResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_page_maps_to_offset_page() {
        let page = ProviderPage { items: vec![1, 2, 3], next: Some("x".into()), total: Some(10) };
        let p = page.into_page(20);
        assert_eq!(p.next_offset, Some(23));
        assert_eq!(p.total, 10);
        let last = ProviderPage { items: vec![1], next: None, total: None }.into_page(5);
        assert_eq!((last.next_offset, last.total), (None, 6));
    }
}
