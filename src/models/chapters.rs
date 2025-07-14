use serde::{Deserialize, Serialize};

/// Deserialize chapters information
/// It goes 2 levels deep which works for Sandersons books which is all I want but there could be
/// books with more levels
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudibleChapters {
    #[serde(rename = "content_metadata")]
    pub content_metadata: ContentMetadata,
    #[serde(rename = "response_groups")]
    pub response_groups: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentMetadata {
    #[serde(rename = "chapter_info")]
    pub chapter_info: ChapterInfo,
    #[serde(rename = "content_reference")]
    pub content_reference: ContentReference,
    #[serde(rename = "last_position_heard")]
    pub last_position_heard: LastPositionHeard,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterInfo {
    pub brand_intro_duration_ms: i64,
    pub brand_outro_duration_ms: i64,
    pub chapters: Vec<Chapter>,
    #[serde(rename = "is_accurate")]
    pub is_accurate: bool,
    #[serde(rename = "runtime_length_ms")]
    pub runtime_length_ms: i64,
    #[serde(rename = "runtime_length_sec")]
    pub runtime_length_sec: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chapter {
    #[serde(rename = "length_ms")]
    pub length_ms: i64,
    #[serde(rename = "start_offset_ms")]
    pub start_offset_ms: i64,
    #[serde(rename = "start_offset_sec")]
    pub start_offset_sec: i64,
    pub title: String,
    #[serde(default)]
    pub chapters: Vec<Chapter2>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chapter2 {
    #[serde(rename = "length_ms")]
    pub length_ms: i64,
    #[serde(rename = "start_offset_ms")]
    pub start_offset_ms: i64,
    #[serde(rename = "start_offset_sec")]
    pub start_offset_sec: i64,
    pub title: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentReference {
    pub acr: String,
    pub asin: String,
    pub codec: String,
    #[serde(rename = "content_format")]
    pub content_format: String,
    #[serde(rename = "content_size_in_bytes")]
    pub content_size_in_bytes: i64,
    #[serde(rename = "file_version")]
    pub file_version: String,
    pub marketplace: String,
    pub sku: String,
    pub tempo: String,
    pub version: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastPositionHeard {
    #[serde(rename = "last_updated")]
    pub last_updated: String,
    #[serde(rename = "position_ms")]
    pub position_ms: i64,
    pub status: String,
}
