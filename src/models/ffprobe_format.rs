use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FFProbeFormat {
    pub format: Format,
}

impl FFProbeFormat {
    pub fn validate(&self) -> Result<(), String> {
        self.format.validate().map_err(|e| format!("format: {}", e))
    }
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

impl Format {
    pub fn validate(&self) -> Result<(), String> {
        if self.filename.trim().is_empty() { return Err("filename is empty".to_string()); }
        if self.nb_streams <= 0 { return Err("nb_streams is not positive".to_string()); }
        if self.format_name.trim().is_empty() { return Err("format_name is empty".to_string()); }
        if self.format_long_name.trim().is_empty() { return Err("format_long_name is empty".to_string()); }
        if self.start_time.trim().is_empty() { return Err("start_time is empty".to_string()); }
        if self.duration.trim().is_empty() { return Err("duration is empty".to_string()); }
        if self.size.trim().is_empty() { return Err("size is empty".to_string()); }
        if self.bit_rate.trim().is_empty() { return Err("bit_rate is empty".to_string()); }
        if self.probe_score < 0 { return Err("probe_score is negative".to_string()); }
        self.tags.validate().map_err(|e| format!("tags: {}", e))?;
        Ok(())
    }
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

impl Tags {
    pub fn validate(&self) -> Result<(), String> {
        if self.major_brand.trim().is_empty() { return Err("major_brand is empty".to_string()); }
        if self.minor_version.trim().is_empty() { return Err("minor_version is empty".to_string()); }
        if self.compatible_brands.trim().is_empty() { return Err("compatible_brands is empty".to_string()); }
        if self.creation_time.trim().is_empty() { return Err("creation_time is empty".to_string()); }
        if self.genre.trim().is_empty() { return Err("genre is empty".to_string()); }
        if self.title.trim().is_empty() { return Err("title is empty".to_string()); }
        if self.artist.trim().is_empty() { return Err("artist is empty".to_string()); }
        if self.album_artist.trim().is_empty() { return Err("album_artist is empty".to_string()); }
        if self.album.trim().is_empty() { return Err("album is empty".to_string()); }
        if self.comment.trim().is_empty() { return Err("comment is empty".to_string()); }
        if self.copyright.trim().is_empty() { return Err("copyright is empty".to_string()); }
        if self.date.trim().is_empty() { return Err("date is empty".to_string()); }
        Ok(())
    }
}

