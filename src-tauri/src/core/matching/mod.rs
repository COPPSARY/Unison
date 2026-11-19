//! Cross-platform song identity (spec §8.1–§8.2): a normaliser and a matcher that decides
//! whether a new source item is the same song as an existing `tracks` row.

pub mod matcher;
pub mod normalize;
