use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Deserialize chapters information with recursive structure to handle unlimited nesting levels
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudibleChapters {
    #[serde(rename = "content_metadata")]
    pub content_metadata: ContentMetadata,
    #[serde(rename = "response_groups")]
    pub response_groups: Vec<String>,
}

impl AudibleChapters {
    pub fn validate(&self) -> Result<(), String> {
        self.content_metadata.validate().map_err(|e| format!("content_metadata: {}", e))?;
        if self.response_groups.is_empty() {
            return Err("response_groups is empty".to_string());
        }
        Ok(())
    }
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

impl ContentMetadata {
    pub fn validate(&self) -> Result<(), String> {
        self.chapter_info.validate().map_err(|e| format!("chapter_info: {}", e))?;
        self.content_reference.validate().map_err(|e| format!("content_reference: {}", e))?;
        self.last_position_heard.validate().map_err(|e| format!("last_position_heard: {}", e))?;
        Ok(())
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterInfo {
    #[serde(rename = "brandIntroDurationMs")]
    pub brand_intro_duration_ms: i64,
    #[serde(rename = "brandOutroDurationMs")]
    pub brand_outro_duration_ms: i64,
    pub chapters: Vec<ChapterNode>,
    #[serde(rename = "is_accurate")]
    pub is_accurate: bool,
    #[serde(rename = "runtime_length_ms")]
    pub runtime_length_ms: i64,
    #[serde(rename = "runtime_length_sec")]
    pub runtime_length_sec: i64,
}

impl ChapterInfo {
    pub fn validate(&self) -> Result<(), String> {
        if self.brand_intro_duration_ms < 0 { return Err("brand_intro_duration_ms is negative".to_string()); }
        if self.brand_outro_duration_ms < 0 { return Err("brand_outro_duration_ms is negative".to_string()); }
        if self.runtime_length_ms <= 0 { return Err("runtime_length_ms is not positive".to_string()); }
        if self.runtime_length_sec <= 0 { return Err("runtime_length_sec is not positive".to_string()); }
        for (i, chapter) in self.chapters.iter().enumerate() {
            chapter.validate().map_err(|e| format!("chapters[{}]: {}", i, e))?;
        }
        Ok(())
    }
}

/// Recursive chapter structure that can handle unlimited nesting levels
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterNode {
    #[serde(rename = "length_ms")]
    pub length_ms: i64,
    #[serde(rename = "start_offset_ms")]
    pub start_offset_ms: i64,
    #[serde(rename = "start_offset_sec")]
    pub start_offset_sec: i64,
    pub title: String,
    #[serde(default)]
    pub chapters: Vec<ChapterNode>,
}

impl ChapterNode {
    pub fn validate(&self) -> Result<(), String> {
        if self.title.trim().is_empty() { return Err("title is empty".to_string()); }
        if self.length_ms <= 0 { return Err("length_ms is not positive".to_string()); }
        if self.start_offset_ms < 0 { return Err("start_offset_ms is negative".to_string()); }
        if self.start_offset_sec < 0 { return Err("start_offset_sec is negative".to_string()); }
        for (i, chapter) in self.chapters.iter().enumerate() {
            chapter.validate().map_err(|e| format!("chapters[{}]: {}", i, e))?;
        }
        Ok(())
    }

    /// Flatten the hierarchical chapter structure into a flat list
    pub fn flatten(&self) -> Vec<FlattenedChapter> {
        let mut result = Vec::new();
        let mut chapter_counter = 1;
        
        self.flatten_recursive(&mut result, &mut chapter_counter, String::new(), 0);
        result
    }
    
    pub fn flatten_recursive(
        &self, 
        result: &mut Vec<FlattenedChapter>,
        counter: &mut usize,
        parent_path: String,
        level: usize
    ) {
        // Build full path
        let full_path = if parent_path.is_empty() {
            self.title.clone()
        } else {
            format!("{} > {}", parent_path, self.title)
        };
        
        // Add this chapter if it's a leaf chapter (no children)
        // Parent chapters with children are handled by their sub-chapters
        if self.chapters.is_empty() {
            // Create a hierarchical title that includes parent context
            let hierarchical_title = if parent_path.is_empty() {
                self.title.clone()
            } else {
                // Convert "Part One: Empire > Chapter 1" to "Part_One_Empire_Chapter_1"
                let path_parts: Vec<&str> = full_path.split(" > ").collect();
                path_parts.join("_")
                    .replace(":", "")
                    .replace(" ", "_")
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                    .collect()
            };
            
            result.push(FlattenedChapter {
                title: hierarchical_title,
                full_path: full_path.clone(),
                start_offset_ms: self.start_offset_ms,
                length_ms: self.length_ms,
                start_offset_sec: self.start_offset_sec,
                level,
                chapter_number: *counter,
            });
            *counter += 1;
        } else if self.length_ms > 0 {
            // This is a parent chapter with its own content - add it as a content chapter
            // but don't include it in the hierarchical path for its children
            // The parent chapter should be placed in its own directory
            let hierarchical_title = if parent_path.is_empty() {
                self.title.clone()
            } else {
                // Convert "Part One: Empire > Chapter 1" to "Part_One_Empire_Chapter_1"
                let path_parts: Vec<&str> = full_path.split(" > ").collect();
                path_parts.join("_")
                    .replace(":", "")
                    .replace(" ", "_")
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                    .collect()
            };
            
            // For parent chapters with content, we need to create a special full_path
            // that will place them in their own directory
            // The parent chapter should be treated as if it has no parent path
            // so it gets placed in its own directory
            let parent_full_path = self.title.clone();
            
            result.push(FlattenedChapter {
                title: hierarchical_title,
                full_path: parent_full_path,
                start_offset_ms: self.start_offset_ms,
                length_ms: self.length_ms,
                start_offset_sec: self.start_offset_sec,
                level,
                chapter_number: *counter,
            });
            *counter += 1;
        }
        
        // Recursively process children
        for child in &self.chapters {
            child.flatten_recursive(result, counter, full_path.clone(), level + 1);
        }
    }
}

/// A flattened chapter with metadata for file generation
#[derive(Debug, Clone, PartialEq)]
pub struct FlattenedChapter {
    pub title: String,
    pub full_path: String,        // e.g., "Part 1 > Chapter 01"
    pub start_offset_ms: i64,
    pub length_ms: i64,
    pub start_offset_sec: i64,
    pub level: usize,             // How deep in the hierarchy
    pub chapter_number: usize,    // Sequential number for naming (starts from 1)
}

/// Represents a chapter that may have been merged with previous short chapters
#[derive(Debug, Clone, PartialEq)]
pub struct MergedChapter {
    pub title: String,
    pub full_path: String,
    pub start_offset_ms: i64,
    pub length_ms: i64,
    pub start_offset_sec: i64,
    pub level: usize,
    pub chapter_number: usize,
    pub merged_chapters: Vec<String>, // Titles of chapters that were merged into this one
}

impl MergedChapter {
    /// Create a MergedChapter from a FlattenedChapter
    pub fn from_flattened(chapter: &FlattenedChapter) -> Self {
        Self {
            title: chapter.title.clone(),
            full_path: chapter.full_path.clone(),
            start_offset_ms: chapter.start_offset_ms,
            length_ms: chapter.length_ms,
            start_offset_sec: chapter.start_offset_sec,
            level: chapter.level,
            chapter_number: chapter.chapter_number,
            merged_chapters: vec![chapter.title.clone()],
        }
    }
    
    /// Merge another chapter into this one
    pub fn merge_with(&mut self, other: &FlattenedChapter) {
        // Extend the length to include the other chapter
        let other_end = other.start_offset_ms + other.length_ms;
        self.length_ms = other_end - self.start_offset_ms;
        self.length_ms = self.length_ms.max(other.length_ms); // Ensure we don't go backwards
        
        // Add the other chapter's title to merged chapters
        self.merged_chapters.push(other.title.clone());
        
        // Update the title to indicate merging
        if self.merged_chapters.len() > 1 {
            self.title = format!("{} (includes: {})", 
                self.merged_chapters[0], 
                self.merged_chapters[1..].join(", "));
        }
    }
    
    /// Generate filename based on format pattern
    pub fn generate_filename(&self, format: &ChapterNamingFormat, extension: &str) -> String {
        match format {
            ChapterNamingFormat::ChapterNumberTitle => {
                format!("Chapter{:02}_{}.{}", 
                    self.chapter_number, 
                    self.sanitize_title(&self.title), 
                    extension)
            },
            ChapterNamingFormat::NumberTitle => {
                format!("{:02}_{}.{}", 
                    self.chapter_number, 
                    self.sanitize_title(&self.title), 
                    extension)
            },
            ChapterNamingFormat::TitleOnly => {
                format!("{}.{}", 
                    self.sanitize_title(&self.title), 
                    extension)
            },
            ChapterNamingFormat::Custom(pattern) => {
                pattern
                    .replace("{chapter:02}", &format!("{:02}", self.chapter_number))
                    .replace("{chapter}", &format!("{}", self.chapter_number))
                    .replace("{number:02}", &format!("{:02}", self.chapter_number))
                    .replace("{number}", &format!("{}", self.chapter_number))
                    .replace("{title}", &self.sanitize_title(&self.title))
                    .replace("{extension}", extension)
            }
        }
    }
    
    /// Sanitize title for use in filename
    fn sanitize_title(&self, title: &str) -> String {
        title
            .replace(":", "")
            .replace("/", "_")
            .replace("\\", "_")
            .replace("?", "")
            .replace("*", "")
            .replace("\"", "")
            .replace("<", "")
            .replace(">", "")
            .replace("|", "")
            .replace(" ", "_")
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect()
    }
    
    /// Get hierarchical output path for this chapter
    pub fn get_hierarchical_output_path(&self, base_path: &Path, format: &ChapterNamingFormat, extension: &str) -> PathBuf {
        let filename = self.generate_filename(format, extension);
        
        // Parse the full_path to create directory structure
        // e.g., "Part One: Empire > Chapter 1" -> "Part_One_Empire/Chapter_1.mp3"
        let path_parts: Vec<&str> = self.full_path.split(" > ").collect();
        
        if path_parts.len() <= 1 {
            // No hierarchy - check if this is a parent chapter that should be in its own directory
            // If the title contains the chapter number pattern, it's a parent chapter
            if filename.contains("Chapter") && !self.full_path.contains(" > ") {
                // This is a parent chapter with content - place it in its own directory
                let dir_name = self.full_path
                    .replace(":", "")
                    .replace(" ", "_")
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                    .collect::<String>();
                base_path.join(dir_name).join(filename)
            } else {
                // Regular top-level chapter
                base_path.join(filename)
            }
        } else {
            // Create directory structure from parent parts
            let parent_parts = &path_parts[..path_parts.len() - 1];
            let mut path = base_path.to_path_buf();
            
            for part in parent_parts {
                let dir_name = part
                    .replace(":", "")
                    .replace(" ", "_")
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                    .collect::<String>();
                path.push(dir_name);
            }
            
            // Add the filename
            path.push(filename);
            path
        }
    }
}

impl FlattenedChapter {
    /// Check if this chapter should be included based on minimum duration
    pub fn should_include(&self, min_duration_ms: i64) -> bool {
        self.length_ms >= min_duration_ms
    }
    
    /// Check if this chapter should be merged with the next chapter
    pub fn should_merge_with_next(&self, min_duration_ms: i64) -> bool {
        self.length_ms < min_duration_ms && self.length_ms > 0
    }
    
    /// Generate filename based on format pattern
    pub fn generate_filename(&self, format: &ChapterNamingFormat, extension: &str) -> String {
        match format {
            ChapterNamingFormat::ChapterNumberTitle => {
                format!("Chapter{:02}_{}.{}", 
                    self.chapter_number, 
                    self.sanitize_title(&self.title),
                    extension
                )
            },
            ChapterNamingFormat::NumberTitle => {
                format!("{:02}_{}.{}", 
                    self.chapter_number, 
                    self.sanitize_title(&self.title),
                    extension
                )
            },
            ChapterNamingFormat::TitleOnly => {
                format!("{}.{}", 
                    self.sanitize_title(&self.title),
                    extension
                )
            },
            ChapterNamingFormat::Custom(pattern) => {
                pattern
                    .replace("{number:02}", &format!("{:02}", self.chapter_number))
                    .replace("{number}", &format!("{}", self.chapter_number))
                    .replace("{title}", &self.sanitize_title(&self.title))
                    .replace("{extension}", extension)
            }
        }
    }
    
    /// Sanitize title for use in filename
    fn sanitize_title(&self, title: &str) -> String {
        title
            .replace(":", "")
            .replace("/", "_")
            .replace("\\", "_")
            .replace("?", "")
            .replace("*", "")
            .replace("\"", "")
            .replace("<", "")
            .replace(">", "")
            .replace("|", "")
            .replace(" ", "_")
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect()
    }
    
    /// Get output path for this chapter
    pub fn get_output_path(&self, base_path: &PathBuf, format: &ChapterNamingFormat, extension: &str) -> PathBuf {
        let filename = self.generate_filename(format, extension);
        base_path.join(filename)
    }
    
    /// Get hierarchical output path for this chapter
    pub fn get_hierarchical_output_path(&self, base_path: &Path, format: &ChapterNamingFormat, extension: &str) -> PathBuf {
        let filename = self.generate_filename(format, extension);
        
        // Parse the full_path to create directory structure
        // e.g., "Part One: Empire > Chapter 1" -> "Part_One_Empire/Chapter_1.mp3"
        let path_parts: Vec<&str> = self.full_path.split(" > ").collect();
        
        if path_parts.len() <= 1 {
            // No hierarchy - check if this is a parent chapter that should be in its own directory
            // If the title contains the chapter number pattern, it's a parent chapter
            if filename.contains("Chapter") && !self.full_path.contains(" > ") {
                // This is a parent chapter with content - place it in its own directory
                let dir_name = self.full_path
                    .replace(":", "")
                    .replace(" ", "_")
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                    .collect::<String>();
                base_path.join(dir_name).join(filename)
            } else {
                // Regular top-level chapter
                base_path.join(filename)
            }
        } else {
            // Create directory structure from parent parts
            let parent_parts = &path_parts[..path_parts.len() - 1];
            let mut path = base_path.to_path_buf();
            
            for part in parent_parts {
                let dir_name = part
                    .replace(":", "")
                    .replace(" ", "_")
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                    .collect::<String>();
                path.push(dir_name);
            }
            
            // Add the filename
            path.push(filename);
            path
        }
    }
}

/// Chapter naming format options
#[derive(Debug, Clone, PartialEq)]
pub enum ChapterNamingFormat {
    /// Chapter01_Title.ext
    ChapterNumberTitle,
    /// 01_Title.ext
    NumberTitle,
    /// Title.ext
    TitleOnly,
    /// Custom pattern with placeholders: {number:02}, {number}, {title}, {extension}
    Custom(String),
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

impl ContentReference {
    pub fn validate(&self) -> Result<(), String> {
        if self.acr.trim().is_empty() { return Err("acr is empty".to_string()); }
        if self.asin.trim().is_empty() { return Err("asin is empty".to_string()); }
        if self.codec.trim().is_empty() { return Err("codec is empty".to_string()); }
        if self.content_format.trim().is_empty() { return Err("content_format is empty".to_string()); }
        if self.content_size_in_bytes <= 0 { return Err("content_size_in_bytes is not positive".to_string()); }
        if self.file_version.trim().is_empty() { return Err("file_version is empty".to_string()); }
        if self.marketplace.trim().is_empty() { return Err("marketplace is empty".to_string()); }
        if self.sku.trim().is_empty() { return Err("sku is empty".to_string()); }
        if self.tempo.trim().is_empty() { return Err("tempo is empty".to_string()); }
        if self.version.trim().is_empty() { return Err("version is empty".to_string()); }
        Ok(())
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastPositionHeard {
    #[serde(rename = "last_updated")]
    pub last_updated: Option<String>,
    #[serde(rename = "position_ms")]
    pub position_ms: Option<i64>,
    pub status: String,
}

impl LastPositionHeard {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(ref last_updated) = self.last_updated {
            if last_updated.trim().is_empty() { return Err("last_updated is empty".to_string()); }
        }
        if let Some(position_ms) = self.position_ms {
            if position_ms < 0 { return Err("position_ms is negative".to_string()); }
        }
        if self.status.trim().is_empty() { return Err("status is empty".to_string()); }
        Ok(())
    }
}
