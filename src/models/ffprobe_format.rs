use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FFProbeFormat {
    pub format: Format,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Format {
    pub filename: String,
    #[serde(rename = "nb_streams")]
    pub nb_streams: i64,
    #[serde(rename = "nb_programs")]
    pub nb_programs: i64,
    #[serde(rename = "nb_stream_groups")]
    pub nb_stream_groups: i64,
    #[serde(rename = "format_name")]
    pub format_name: String,
    #[serde(rename = "format_long_name")]
    pub format_long_name: String,
    #[serde(rename = "start_time")]
    pub start_time: String,
    pub duration: String,
    pub size: String,
    #[serde(rename = "bit_rate")]
    pub bit_rate: String,
    #[serde(rename = "probe_score")]
    pub probe_score: i64,
    pub tags: Tags,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tags {
    #[serde(rename = "major_brand")]
    pub major_brand: String,
    #[serde(rename = "minor_version")]
    pub minor_version: String,
    #[serde(rename = "compatible_brands")]
    pub compatible_brands: String,
    #[serde(rename = "creation_time")]
    pub creation_time: String,
    pub genre: String,
    pub title: String,
    pub artist: String,
    #[serde(rename = "album_artist")]
    pub album_artist: String,
    pub album: String,
    pub comment: String,
    pub copyright: String,
    pub date: String,
}
