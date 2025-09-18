mod cli;
mod models;

use crate::models::{FFProbeFormat, AudibleChapters, FlattenedChapter, MergedChapter, ChapterNamingFormat};
use crate::cli::SplitStructure;
use clap::Parser;
use inflector::Inflector;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::{
    io::BufRead,
    process::{Command, Stdio},
};
use anyhow::{Context, Result};
use log::{info, error, warn};
use indicatif::{ProgressBar, ProgressStyle};

fn main() -> Result<()> {
    // Initialize logger
    env_logger::init();

    info!("Starting audible-util");
    if let Err(e) = run() {
        error!("Fatal error: {e}");
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
    info!("audible-util finished successfully");
    Ok(())
}

fn run() -> Result<()> {
    info!("Parsing CLI arguments");

    let cli = cli::Cli::parse();

    // --- Early input validation ---

    // Check input .aaxc file exists, is readable, and has correct extension
    let aaxc_file_path = cli.aaxc_path;
    if !aaxc_file_path.exists() {
        anyhow::bail!(
            "Input file does not exist: {}. Please provide a valid .aaxc file.",
            aaxc_file_path.display()
            );
    }
    if !aaxc_file_path.is_file() {
        anyhow::bail!(
            "Input path is not a file: {}. Please provide a valid .aaxc file.",
            aaxc_file_path.display()
        );
    }
    if aaxc_file_path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()) != Some("aaxc".to_string()) {
        anyhow::bail!(
            "Input file does not have a .aaxc extension: {}. Please provide a valid Audible .aaxc file.",
            aaxc_file_path.display()
        );
    }
    if std::fs::File::open(&aaxc_file_path).is_err() {
        anyhow::bail!(
            "Input file is not readable: {}. Please check file permissions.",
            aaxc_file_path.display()
        );
    }

    // Determine voucher file path: use CLI override if provided
    let voucher_file_path = if let Some(voucher_path) = cli.voucher_path {
        info!("Using voucher file from CLI: {}", voucher_path.display());
        // Check voucher file exists and is readable
        if !voucher_path.exists() {
            anyhow::bail!(
                "Voucher file does not exist: {}. Please provide a valid voucher file.",
                voucher_path.display()
            );
        }
        if !voucher_path.is_file() {
            anyhow::bail!(
                "Voucher path is not a file: {}. Please provide a valid voucher file.",
                voucher_path.display()
            );
        }
        if std::fs::File::open(&voucher_path).is_err() {
            anyhow::bail!(
                "Voucher file is not readable: {}. Please check file permissions.",
                voucher_path.display()
            );
        }
        voucher_path
    } else {
        let aaxc_file_path_stem = aaxc_file_path
            .file_stem()
            .context("Could not get file stem from the input file path. Please provide a valid .aaxc file.")?;
        let path = aaxc_file_path.with_file_name(
            format!(
                "{}.voucher",
                aaxc_file_path_stem
                    .to_str()
                    .context("Failed to convert file stem to string. Please check your input file name.")?
            ),
        );
        info!("Using inferred voucher file: {}", path.display());
        if !path.exists() {
            anyhow::bail!(
                "Inferred voucher file does not exist: {}. Please provide a valid voucher file or use --voucher-path.",
                path.display()
            );
        }
        if !path.is_file() {
            anyhow::bail!(
                "Inferred voucher path is not a file: {}. Please provide a valid voucher file or use --voucher-path.",
                path.display()
            );
        }
        if std::fs::File::open(&path).is_err() {
            anyhow::bail!(
                "Inferred voucher file is not readable: {}. Please check file permissions or use --voucher-path.",
                path.display()
            );
        }
        path
    };

    // If output path is provided, check parent directory exists and is writable
    if let Some(ref output_path) = cli.output_path {
    
        if output_path.exists() && output_path.is_dir() {
            // If output_path is a directory, check if it's writable
            if std::fs::metadata(output_path)
                .map(|m| m.permissions().readonly())
                .unwrap_or(true)
            {
                anyhow::bail!(
                    "Output directory is not writable: {}. Please check permissions or specify a different output path.",
                    output_path.display()
                );
            }
        } else if !output_path.exists() {
            // If output_path does not exist, try to create it as a directory
            if let Err(e) = std::fs::create_dir_all(output_path) {
                anyhow::bail!(
                    "Failed to create output directory '{}': {}. Please check permissions or specify a different output path.",
                    output_path.display(),
                    e
                );
            }
            if std::fs::metadata(output_path)
                .map(|m| m.permissions().readonly())
                .unwrap_or(true)
            {
                anyhow::bail!(
                    "Output directory is not writable: {}. Please check permissions or specify a different output path.",
                    output_path.display()
                );
            }
        } else if let Some(parent) = output_path.parent() {
            // If output_path is a file path, ensure its parent directory exists or create it
            if !parent.exists() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    anyhow::bail!(
                        "Failed to create output directory '{}': {}. Please check permissions or specify a different output path.",
                        parent.display(),
                        e
                    );
                }
            }
            if std::fs::metadata(parent)
                .map(|m| m.permissions().readonly())
                .unwrap_or(true)
            {
                anyhow::bail!(
                    "Output directory is not writable: {}. Please check permissions or specify a different output path.",
                    parent.display()
                );
            }
        }
    }

    // --- Pre-flight checks for ffmpeg and ffprobe ---
    check_external_tool("ffmpeg")?;
    check_external_tool("ffprobe")?;

    // Use serde to deserialize voucher file into `AudibleCliVoucher`
    info!("Opening voucher file: {}", voucher_file_path.display());
    let voucher_file = std::fs::File::open(&voucher_file_path)
        .with_context(|| format!(
            "Failed to open voucher file: {}. Please ensure the file exists and is readable.",
            voucher_file_path.display()
        ))?;
    info!("Parsing voucher file");
    let voucher: models::AudibleCliVoucher = serde_json::from_reader(voucher_file)
        .with_context(|| format!(
            "Failed to parse voucher file: {}. Please ensure it is a valid JSON file generated by audible-cli.",
            voucher_file_path.display()
        ))?;
    voucher.validate().map_err(|e| anyhow::anyhow!("Invalid voucher: {e}"))?;
    info!("Voucher validated successfully");

    let audible_key = voucher.content_license.license_response.key;
    let audible_iv = voucher.content_license.license_response.iv;

    info!("Running ffprobe on input file: {}", aaxc_file_path.display());
    let ffprobe_json = ffprobe(&aaxc_file_path)
        .with_context(|| format!(
            "Failed to probe input file: {}. Please ensure ffprobe is installed and the file is a valid Audible AAXC file.",
            aaxc_file_path.display()
        ))?;
    ffprobe_json.validate().map_err(|e| anyhow::anyhow!("Invalid ffprobe data: {e}"))?;
    info!("ffprobe completed and validated");

    let title = ffprobe_json.format.tags.title;
    let album = ffprobe_json.format.tags.album;
    let duration = ffprobe_json.format.duration;

    // Determine output file extension and codec based on output_type (trait-based, extensible)
    use crate::cli::OutputFormat;
    let output_format: Box<dyn OutputFormat> = cli.output_type.get_format();
    let codec = output_format.codec();
    let ext = output_format.extension();

    // Determine output file name: use CLI override if provided
    let file_name = if let Some(ref output_path) = cli.output_path {
        let default_name = format!("{}.{}", album.to_snake_case(), ext);
        // If the path exists and is a directory, or if it was just created as a directory, use default filename inside it
        if output_path.exists() && output_path.is_dir() {
            output_path.join(&default_name).to_string_lossy().to_string()
        } else if !output_path.exists() {
            // If the path does not exist, check if it was intended as a directory (created above)
            if std::fs::metadata(&output_path).map(|m| m.is_dir()).unwrap_or(false) {
                output_path.join(&default_name).to_string_lossy().to_string()
            } else {
                output_path.to_string_lossy().to_string()
            }
        } else {
            // If output_path is a file path, use as-is
            output_path.to_string_lossy().to_string()
        }
    } else {
        format!("{}.{}", album.to_snake_case(), ext)
    };

    // Handle chapter splitting
    if cli.split {
        info!("Chapter splitting requested");
        
        // Determine chapter file path (similar to voucher file inference)
        let chapter_file_path = {
            let aaxc_file_path_stem = aaxc_file_path
                .file_stem()
                .context("Could not get file stem from the input file path for chapter file inference.")?;
            
            // Try multiple naming patterns for chapter files
            let base_name = aaxc_file_path_stem
                .to_str()
                .context("Failed to convert file stem to string for chapter file inference.")?;
            
            // Remove AAX suffix if present (e.g., "Book-AAX_44_128" -> "Book")
            let clean_name = if base_name.contains("-AAX_") {
                base_name.split("-AAX_").next().unwrap_or(base_name)
            } else {
                base_name
            };
            
            aaxc_file_path.with_file_name(format!("{}-chapters.json", clean_name))
        };
        
        info!("Looking for chapter file: {}", chapter_file_path.display());
        
        // Check if chapter file exists
        if !chapter_file_path.exists() {
            anyhow::bail!(
                "Chapter file does not exist: {}. Please provide a chapters.json file or disable --split.",
                chapter_file_path.display()
            );
        }
        
        if !chapter_file_path.is_file() {
            anyhow::bail!(
                "Chapter path is not a file: {}. Please provide a valid chapters.json file.",
                chapter_file_path.display()
            );
        }
        
        if std::fs::File::open(&chapter_file_path).is_err() {
            anyhow::bail!(
                "Chapter file is not readable: {}. Please check file permissions.",
                chapter_file_path.display()
            );
        }
        
        // Parse chapter file
        info!("Parsing chapter file: {}", chapter_file_path.display());
        let chapter_file = std::fs::File::open(&chapter_file_path)
            .with_context(|| format!(
                "Failed to open chapter file: {}. Please ensure the file exists and is readable.",
                chapter_file_path.display()
            ))?;
        
        let chapters: AudibleChapters = serde_json::from_reader(chapter_file)
            .with_context(|| format!(
                "Failed to parse chapter file: {}. Please ensure it is a valid JSON file.",
                chapter_file_path.display()
            ))?;
        
        info!("Chapter file parsed successfully");
        info!("Response groups: {:?}", chapters.response_groups);
        info!("Chapter count: {}", chapters.content_metadata.chapter_info.chapters.len());
        
        chapters.validate().map_err(|e| anyhow::anyhow!("Invalid chapter data: {e}"))?;
        info!("Chapter data validated successfully");
        
        // Flatten chapters with a single global counter
        let mut flattened_chapters = Vec::new();
        let mut chapter_counter = 1;
        
        for chapter in &chapters.content_metadata.chapter_info.chapters {
            chapter.flatten_recursive(&mut flattened_chapters, &mut chapter_counter, String::new(), 0);
        }
        
        info!("Found {} total chapters", flattened_chapters.len());
        
        // Process chapters based on merging preference
        let min_duration_ms = (cli.min_chapter_duration.unwrap_or(0) * 1000) as i64; // Convert seconds to milliseconds
        let processed_chapters = if cli.merge_short_chapters {
            // Merge short chapters with the next chapter
            let merged_chapters = merge_short_chapters(&flattened_chapters, min_duration_ms);
            info!("After merging short chapters (min duration: {}s): {} chapters", 
                  min_duration_ms / 1000, merged_chapters.len());
            merged_chapters
        } else {
            // Filter chapters based on minimum duration
            let filtered_chapters: Vec<&FlattenedChapter> = flattened_chapters
                .iter()
                .filter(|chapter| chapter.should_include(min_duration_ms))
                .collect();
            
            info!("After filtering (min duration: {}s): {} chapters", 
                  min_duration_ms / 1000, filtered_chapters.len());
            
            if filtered_chapters.is_empty() {
                anyhow::bail!("No chapters found after filtering. Try reducing --min-chapter-duration or check your chapter data.");
            }
            
            // Warn about filtered chapters and potential time gaps
            let filtered_count = flattened_chapters.len() - filtered_chapters.len();
            if filtered_count > 0 {
                warn!("{} chapters were filtered out due to minimum duration requirement. This may create gaps in the audio timeline.", filtered_count);
                warn!("Consider using --merge-short-chapters to merge them with the next chapter instead.");
            }
            
            // Convert to MergedChapter for consistency
            filtered_chapters.into_iter().map(|ch| MergedChapter::from_flattened(ch)).collect()
        };
        
        if processed_chapters.is_empty() {
            anyhow::bail!("No chapters found after processing. Try reducing --min-chapter-duration or check your chapter data.");
        }
        
        // Convert chapters to individual files
        info!("Starting chapter splitting conversion");
        let output_base_path = if let Some(output_path) = &cli.output_path {
            output_path.clone()
        } else {
            PathBuf::from(".")
        };
        convert_chapters(
            &aaxc_file_path,
            &audible_key,
            &audible_iv,
            &processed_chapters,
            &cli.chapter_naming_format,
            &cli.split_structure,
            &output_base_path,
            &ext,
            &codec,
        )?;
        
        info!("Chapter splitting completed successfully");
        return Ok(());
    }

    info!("Title: {}", title);
    info!("Output file name: {}", file_name);

    info!("Starting ffmpeg conversion");
    let mut cmd = ffmpeg(
        aaxc_file_path,
        audible_key,
        audible_iv,
        duration,
        file_name.clone(),
        codec,
    )
    .with_context(|| {
        "Failed to start ffmpeg. Please ensure ffmpeg is installed and available in your PATH."
    })?;

    let status = cmd.wait()
        .with_context(|| "ffmpeg process failed to complete. Please check your input files and try again.")?;

    if status.success() {
        info!("ffmpeg conversion completed successfully");
    } else {
        error!("ffmpeg conversion failed with status: {:?}", status);
        anyhow::bail!(
            "ffmpeg failed to convert the file. Please check your input files and try again. \
If the problem persists, ensure that ffmpeg is installed and supports the required codecs."
        );
    }

    Ok(())
}

fn ffprobe(aaxc_file_path: &Path) -> Result<FFProbeFormat> {
    let ffprobe_cmd = Command::new("ffprobe")
        .args([
            "-i",
            aaxc_file_path
                .to_str()
                .context("Failed to convert input file path to string.")?,
            "-print_format",
            "json",
            "-show_format",
            "-sexagesimal",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| "Failed to execute ffprobe. Is ffprobe installed and available in your PATH?")?;

    if !ffprobe_cmd.status.success() {
        let stderr = String::from_utf8_lossy(&ffprobe_cmd.stderr);
        anyhow::bail!(
            "ffprobe failed with error:\n{}\nPlease ensure the input file is a valid Audible AAXC file.",
            stderr
        );
    }

    let ffprobe_output = std::str::from_utf8(&ffprobe_cmd.stdout)
        .context("Failed to parse ffprobe output as UTF-8.")?;
    let ffprobe_json: models::FFProbeFormat = serde_json::from_str(ffprobe_output)
        .context("Failed to parse ffprobe output as JSON. The file may not be a valid Audible AAXC file.")?;
    Ok(ffprobe_json)
}

/// Merge short chapters with the next chapter
fn merge_short_chapters(chapters: &[FlattenedChapter], min_duration_ms: i64) -> Vec<MergedChapter> {
    let mut merged_chapters = Vec::new();
    let mut i = 0;
    
    while i < chapters.len() {
        let current_chapter = &chapters[i];
        
        if current_chapter.should_include(min_duration_ms) {
            // This chapter is long enough, add it as-is
            merged_chapters.push(MergedChapter::from_flattened(current_chapter));
            i += 1;
        } else if current_chapter.should_merge_with_next(min_duration_ms) {
            // This chapter is short and should be merged with the next
            if i + 1 < chapters.len() {
                // There is a next chapter, merge with it
                let mut merged = MergedChapter::from_flattened(&chapters[i + 1]);
                merged.merge_with(current_chapter);
                merged_chapters.push(merged);
                i += 2; // Skip both the short chapter and the next one
            } else {
                // This is the last chapter and it's short, include it anyway
                merged_chapters.push(MergedChapter::from_flattened(current_chapter));
                i += 1;
            }
        } else {
            // This chapter has no content (length_ms <= 0), skip it
            i += 1;
        }
    }
    
    merged_chapters
}

/// Convert multiple chapters to individual files
fn convert_chapters(
    aaxc_file_path: &Path,
    audible_key: &str,
    audible_iv: &str,
    chapters: &[MergedChapter],
    naming_format: &ChapterNamingFormat,
    split_structure: &SplitStructure,
    output_base_path: &Path,
    extension: &str,
    codec: &str,
) -> Result<()> {
    let total_chapters = chapters.len();
    info!("Converting {} chapters", total_chapters);
    
    for (index, chapter) in chapters.iter().enumerate() {
        let chapter_number = index + 1;
        info!("Converting chapter {}/{}: {}", chapter_number, total_chapters, chapter.title);
        
        // Generate output path based on structure
        let output_path = match split_structure {
            SplitStructure::Flat => {
                let filename = chapter.generate_filename(naming_format, extension);
                output_base_path.join(filename)
            },
            SplitStructure::Hierarchical => {
                chapter.get_hierarchical_output_path(output_base_path, naming_format, extension)
            }
        };
        
        info!("Output file: {}", output_path.display());
        
        // Convert time to ffmpeg format (HH:MM:SS.mmm)
        let start_time = format_time_from_ms(chapter.start_offset_ms);
        let duration_time = format_time_from_ms(chapter.length_ms);
        
        info!("Chapter time range: {} to {} (duration: {})", 
              start_time, 
              format_time_from_ms(chapter.start_offset_ms + chapter.length_ms),
              duration_time);
        
        // Create parent directories if needed
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }
        
        // Run ffmpeg for this chapter
        let mut cmd = ffmpeg_chapter(
            aaxc_file_path.to_path_buf(),
            audible_key.to_string(),
            audible_iv.to_string(),
            start_time,
            duration_time,
            output_path.to_string_lossy().to_string(),
            codec,
        )?;
        
        let status = cmd.wait()
            .with_context(|| format!("ffmpeg process failed for chapter: {}", chapter.title))?;
        
        if status.success() {
            info!("Chapter {}/{} completed: {}", chapter_number, total_chapters, output_path.display());
        } else {
            error!("ffmpeg conversion failed for chapter: {}", chapter.title);
            anyhow::bail!(
                "ffmpeg failed to convert chapter '{}'. Please check your input files and try again.",
                chapter.title
            );
        }
    }
    
    info!("All {} chapters converted successfully", total_chapters);
    Ok(())
}

/// Convert milliseconds to ffmpeg time format (HH:MM:SS.mmm)
fn format_time_from_ms(ms: i64) -> String {
    let total_seconds = ms / 1000;
    let milliseconds = ms % 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    
    format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, seconds, milliseconds)
}

/// Run ffmpeg for a specific chapter with time range
fn ffmpeg_chapter(
    aaxc_file_path: PathBuf,
    audible_key: String,
    audible_iv: String,
    start_time: String,
    duration: String,
    file_name: String,
    codec: &str,
) -> Result<Child> {
    let mut cmd = Command::new("ffmpeg")
        .args([
            "-audible_key",
            audible_key.as_str(),
            "-audible_iv",
            audible_iv.as_str(),
            "-i",
            aaxc_file_path
                .to_str()
                .context("Failed to convert input file path to string.")?,
            "-ss",
            start_time.as_str(),
            "-t",
            duration.as_str(),
            "-progress",
            "/dev/stdout",
            "-y",
            "-map_metadata",
            "0",
            "-vn",
            "-codec:a",
            codec,
            file_name.as_str(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| "Failed to execute ffmpeg. Is ffmpeg installed and available in your PATH?")?;

    {
        let stdout = cmd.stdout.as_mut().context("Failed to capture ffmpeg stdout.")?;
        let stdout_reader = std::io::BufReader::new(stdout);

        // Progress bar setup
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner} [{elapsed_precise}] {msg}")
                .unwrap()
        );
        pb.enable_steady_tick(std::time::Duration::from_millis(100));

        for line in stdout_reader.lines() {
            let l = line.context("Failed to read line from ffmpeg output.")?;
            if l.contains("time=") {
                pb.set_message(format!("Progress: {} / {}", l, duration));
            }
            if l.contains("speed=") {
                pb.set_message(format!("{} | {}", pb.message(), l));
            }
        }
        pb.finish_with_message("Chapter conversion complete");
    }
    info!("ffmpeg process finished for chapter");
    Ok(cmd)
}

fn ffmpeg(
    aaxc_file_path: PathBuf,
    audible_key: String,
    audible_iv: String,
    duration: String,
    file_name: String,
    codec: &str,
) -> Result<Child> {
    let mut cmd = Command::new("ffmpeg")
        .args([
            "-audible_key",
            audible_key.as_str(),
            "-audible_iv",
            audible_iv.as_str(),
            "-i",
            aaxc_file_path
                .to_str()
                .context("Failed to convert input file path to string.")?,
            "-progress",
            "/dev/stdout",
            "-y",
            "-map_metadata",
            "0",
            "-vn",
            "-codec:a",
            codec,
            file_name.as_str(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| "Failed to execute ffmpeg. Is ffmpeg installed and available in your PATH?")?;

    {
        let stdout = cmd.stdout.as_mut().context("Failed to capture ffmpeg stdout.")?;
        let stdout_reader = std::io::BufReader::new(stdout);

        // Progress bar setup
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner} [{elapsed_precise}] {msg}")
                .unwrap()
        );
        pb.enable_steady_tick(std::time::Duration::from_millis(100));

        for line in stdout_reader.lines() {
            let l = line.context("Failed to read line from ffmpeg output.")?;
            if l.contains("time=") {
                pb.set_message(format!("Progress: {} / {}", l, duration));
            }
            if l.contains("speed=") {
                pb.set_message(format!("{} | {}", pb.message(), l));
            }
        }
        pb.finish_with_message("Conversion complete");
    }
    info!("ffmpeg process finished");
    Ok(cmd)
}

/// Checks if an external tool is available in PATH, returns error with guidance if not.
fn check_external_tool(tool: &str) -> Result<()> {
    if which::which(tool).is_err() {
        anyhow::bail!(
            "Required external tool '{}' is not installed or not found in your PATH.\n\
            Please install '{}' and ensure it is available in your system PATH.\n\
            See the README for installation instructions.",
            tool, tool
        );
    }
    Ok(())
}
