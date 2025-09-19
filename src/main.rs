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
use indicatif::{ProgressBar, ProgressStyle, MultiProgress};
use std::time::{Duration, Instant};
use serde::Serialize;

/// Machine-readable progress events for JSON output
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
enum ProgressEvent {
    #[serde(rename = "conversion_started")]
    ConversionStarted {
        total_chapters: usize,
        output_format: String,
        output_path: String,
    },
    #[serde(rename = "chapter_started")]
    ChapterStarted {
        chapter_number: usize,
        total_chapters: usize,
        chapter_title: String,
        duration_seconds: f64,
    },
    #[serde(rename = "chapter_progress")]
    ChapterProgress {
        chapter_number: usize,
        total_chapters: usize,
        chapter_title: String,
        progress_percentage: f64,
        current_time: f64,
        total_duration: f64,
        speed: f64,
        bitrate: f64,
        file_size: u64,
        fps: f64,
        eta_seconds: Option<f64>,
    },
    #[serde(rename = "chapter_completed")]
    ChapterCompleted {
        chapter_number: usize,
        total_chapters: usize,
        chapter_title: String,
        output_file: String,
        duration_seconds: f64,
    },
    #[serde(rename = "conversion_completed")]
    ConversionCompleted {
        total_chapters: usize,
        total_duration_seconds: f64,
        success: bool,
    },
    #[serde(rename = "error")]
    Error {
        message: String,
        chapter_number: Option<usize>,
    },
}

impl ProgressEvent {
    fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Progress tracking information for a single conversion
#[derive(Debug, Clone)]
struct ConversionProgress {
    current_time: f64,
    total_duration: f64,
    speed: f64,
    bitrate: f64,
    size: u64,
    fps: f64,
}

impl ConversionProgress {
    fn new(total_duration: f64) -> Self {
        Self {
            current_time: 0.0,
            total_duration,
            speed: 0.0,
            bitrate: 0.0,
            size: 0,
            fps: 0.0,
        }
    }

    fn percentage(&self) -> f64 {
        if self.total_duration > 0.0 {
            (self.current_time / self.total_duration * 100.0).min(100.0)
        } else {
            0.0
        }
    }

    fn eta(&self) -> Option<Duration> {
        if self.speed > 0.0 && self.current_time < self.total_duration {
            let remaining_time = (self.total_duration - self.current_time) / self.speed;
            Some(Duration::from_secs(remaining_time as u64))
        } else {
            None
        }
    }

    fn format_time(seconds: f64) -> String {
        let hours = (seconds / 3600.0) as u64;
        let minutes = ((seconds % 3600.0) / 60.0) as u64;
        let secs = (seconds % 60.0) as u64;
        format!("{:02}:{:02}:{:02}", hours, minutes, secs)
    }

    fn format_size(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
        let mut size = bytes as f64;
        let mut unit_index = 0;
        
        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }
        
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

/// Progress manager for tracking overall conversion progress
struct ProgressManager {
    multi: MultiProgress,
    overall_pb: ProgressBar,
    current_pb: Option<ProgressBar>,
    start_time: Instant,
    total_chapters: usize,
    current_chapter: usize,
    verbose: bool,
    machine_readable: bool,
}

impl ProgressManager {
    fn new_with_verbose(total_chapters: usize, verbose: bool) -> Self {
        Self::new_with_options(total_chapters, verbose, false)
    }

    fn new_machine_readable(total_chapters: usize) -> Self {
        Self::new_with_options(total_chapters, false, true)
    }

    fn new_with_options(total_chapters: usize, verbose: bool, machine_readable: bool) -> Self {
        let multi = MultiProgress::new();
        let overall_pb = multi.add(ProgressBar::new(total_chapters as u64));
        
        if !machine_readable {
            overall_pb.set_style(
                ProgressStyle::default_bar()
                    .template("{bar:40.cyan/blue} {pos:>3}/{len:3} chapters [{elapsed_precise}] {msg}")
                    .unwrap()
                    .progress_chars("█▉▊▋▌▍▎▏  "),
            );
            overall_pb.set_message("Starting conversion...");
        } else {
            // Hide progress bars in machine-readable mode
            overall_pb.set_style(ProgressStyle::default_bar().template("").unwrap());
        }

        Self {
            multi,
            overall_pb,
            current_pb: None,
            start_time: Instant::now(),
            total_chapters,
            current_chapter: 0,
            verbose,
            machine_readable,
        }
    }

    fn start_chapter(&mut self, chapter_title: &str, duration: f64) -> ProgressBar {
        self.current_chapter += 1;
        
        if self.machine_readable {
            let event = ProgressEvent::ChapterStarted {
                chapter_number: self.current_chapter,
                total_chapters: self.total_chapters,
                chapter_title: chapter_title.to_string(),
                duration_seconds: duration,
            };
            println!("{}", event.to_json());
        } else {
            self.overall_pb.set_message(format!("Chapter {}/{}: {}", 
                self.current_chapter, self.total_chapters, chapter_title));
        }

        let current_pb = self.multi.add(ProgressBar::new(duration as u64));
        
        if !self.machine_readable {
            current_pb.set_style(
                ProgressStyle::default_bar()
                    .template("{bar:40.green/yellow} {percent:>3}% [{elapsed_precise}] {msg}")
                    .unwrap()
                    .progress_chars("█▉▊▋▌▍▎▏  "),
            );
            current_pb.set_message(format!("Converting: {}", chapter_title));
            current_pb.enable_steady_tick(Duration::from_millis(100));
        } else {
            // Hide progress bars in machine-readable mode
            current_pb.set_style(ProgressStyle::default_bar().template("").unwrap());
        }

        self.current_pb = Some(current_pb.clone());
        current_pb
    }

    fn update_chapter_progress(&self, progress: &ConversionProgress) {
        if self.machine_readable {
            let event = ProgressEvent::ChapterProgress {
                chapter_number: self.current_chapter,
                total_chapters: self.total_chapters,
                chapter_title: "".to_string(), // Will be filled by caller
                progress_percentage: progress.percentage(),
                current_time: progress.current_time,
                total_duration: progress.total_duration,
                speed: progress.speed,
                bitrate: progress.bitrate,
                file_size: progress.size,
                fps: progress.fps,
                eta_seconds: progress.eta().map(|eta| eta.as_secs() as f64),
            };
            println!("{}", event.to_json());
        } else {
            if let Some(ref pb) = self.current_pb {
                pb.set_position(progress.current_time as u64);
                
                let eta_str = progress.eta()
                    .map(|eta| format!("ETA: {}", Self::format_duration(eta)))
                    .unwrap_or_else(|| "ETA: --:--:--".to_string());
                
                let speed_str = if progress.speed > 0.0 {
                    format!("Speed: {:.1}x", progress.speed)
                } else {
                    "Speed: --".to_string()
                };
                
                let bitrate_str = if progress.bitrate > 0.0 {
                    format!("Bitrate: {:.0} kbps", progress.bitrate / 1000.0)
                } else {
                    "Bitrate: --".to_string()
                };
                
                let size_str = if progress.size > 0 {
                    format!("Size: {}", ConversionProgress::format_size(progress.size))
                } else {
                    "Size: --".to_string()
                };

                let fps_str = if self.verbose && progress.fps > 0.0 {
                    format!("FPS: {:.1}", progress.fps)
                } else {
                    String::new()
                };

                let time_str = if self.verbose {
                    format!("Time: {}/{}", 
                        ConversionProgress::format_time(progress.current_time),
                        ConversionProgress::format_time(progress.total_duration))
                } else {
                    String::new()
                };

                let mut message_parts = vec![eta_str, speed_str, bitrate_str, size_str];
                if self.verbose {
                    if !fps_str.is_empty() {
                        message_parts.push(fps_str);
                    }
                    if !time_str.is_empty() {
                        message_parts.push(time_str);
                    }
                }

                pb.set_message(message_parts.join(" | "));
            }

            // Log detailed progress in verbose mode
            if self.verbose {
                info!("Progress: {:.1}% | Time: {}/{} | Speed: {:.1}x | Bitrate: {:.0} kbps | Size: {}", 
                    progress.percentage(),
                    ConversionProgress::format_time(progress.current_time),
                    ConversionProgress::format_time(progress.total_duration),
                    progress.speed,
                    progress.bitrate / 1000.0,
                    ConversionProgress::format_size(progress.size)
                );
            }
        }
    }

    fn complete_chapter(&mut self, chapter_title: &str, output_file: &str, duration: f64) {
        if self.machine_readable {
            let event = ProgressEvent::ChapterCompleted {
                chapter_number: self.current_chapter,
                total_chapters: self.total_chapters,
                chapter_title: chapter_title.to_string(),
                output_file: output_file.to_string(),
                duration_seconds: duration,
            };
            println!("{}", event.to_json());
        } else {
            if let Some(pb) = self.current_pb.take() {
                pb.finish_with_message("Chapter completed");
            }
        }
        self.overall_pb.inc(1);
    }

    fn complete_all(&self, success: bool) {
        if self.machine_readable {
            let event = ProgressEvent::ConversionCompleted {
                total_chapters: self.total_chapters,
                total_duration_seconds: self.start_time.elapsed().as_secs() as f64,
                success,
            };
            println!("{}", event.to_json());
        } else {
            self.overall_pb.finish_with_message(format!(
                "All {} chapters completed in {}",
                self.total_chapters,
                Self::format_duration(self.start_time.elapsed())
            ));
        }
    }

    fn emit_error(&self, message: &str, chapter_number: Option<usize>) {
        if self.machine_readable {
            let event = ProgressEvent::Error {
                message: message.to_string(),
                chapter_number,
            };
            println!("{}", event.to_json());
        }
    }

    fn emit_conversion_started(&self, output_format: &str, output_path: &str) {
        if self.machine_readable {
            let event = ProgressEvent::ConversionStarted {
                total_chapters: self.total_chapters,
                output_format: output_format.to_string(),
                output_path: output_path.to_string(),
            };
            println!("{}", event.to_json());
        }
    }

    fn format_duration(duration: Duration) -> String {
        let total_seconds = duration.as_secs();
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    }
}

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
            cli.verbose_progress,
            cli.machine_readable,
            &cli.threads,
        )?;
        
        info!("Chapter splitting completed successfully");
        return Ok(());
    }

    info!("Title: {}", title);
    info!("Output file name: {}", file_name);

    // Handle machine-readable mode for single file conversion
    if cli.machine_readable {
        let event = ProgressEvent::ConversionStarted {
            total_chapters: 1,
            output_format: ext.to_string(),
            output_path: file_name.clone(),
        };
        println!("{}", event.to_json());
    }

    info!("Starting ffmpeg conversion");
    let mut cmd = ffmpeg(
        aaxc_file_path,
        audible_key,
        audible_iv,
        duration,
        file_name.clone(),
        codec,
        cli.verbose_progress,
        cli.machine_readable,
        &cli.threads,
    )
    .with_context(|| {
        "Failed to start ffmpeg. Please ensure ffmpeg is installed and available in your PATH."
    })?;

    let status = cmd.wait()
        .with_context(|| "ffmpeg process failed to complete. Please check your input files and try again.")?;

    if status.success() {
        if cli.machine_readable {
            let event = ProgressEvent::ConversionCompleted {
                total_chapters: 1,
                total_duration_seconds: 0.0, // Will be calculated if needed
                success: true,
            };
            println!("{}", event.to_json());
        }
        info!("ffmpeg conversion completed successfully");
    } else {
        if cli.machine_readable {
            let event = ProgressEvent::Error {
                message: "ffmpeg conversion failed".to_string(),
                chapter_number: Some(1),
            };
            println!("{}", event.to_json());
        }
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
    verbose: bool,
    machine_readable: bool,
    threads: &str,
) -> Result<()> {
    let total_chapters = chapters.len();
    info!("Converting {} chapters", total_chapters);
    
    // Initialize progress manager
    let mut progress_manager = if machine_readable {
        ProgressManager::new_machine_readable(total_chapters)
    } else {
        ProgressManager::new_with_verbose(total_chapters, verbose)
    };

    // Emit conversion started event
    progress_manager.emit_conversion_started(extension, &output_base_path.to_string_lossy());
    
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
        let duration_seconds = chapter.length_ms as f64 / 1000.0;
        
        info!("Chapter time range: {} to {} (duration: {})", 
              start_time, 
              format_time_from_ms(chapter.start_offset_ms + chapter.length_ms),
              duration_time);
        
        // Create parent directories if needed
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }
        
        // Start progress tracking for this chapter
        progress_manager.start_chapter(&chapter.title, duration_seconds);
        
        // Run ffmpeg for this chapter with enhanced progress tracking
        let mut cmd = ffmpeg_chapter_with_progress(
            aaxc_file_path.to_path_buf(),
            audible_key.to_string(),
            audible_iv.to_string(),
            start_time,
            duration_time,
            output_path.to_string_lossy().to_string(),
            codec,
            &progress_manager,
            threads,
        )?;
        
        // Parse ffmpeg progress in the main thread
        if let Some(stdout) = cmd.stdout.as_mut() {
            let stdout_reader = std::io::BufReader::new(stdout);
            let mut progress = ConversionProgress::new(duration_seconds);

            for line in stdout_reader.lines() {
                if let Ok(l) = line {
                    parse_ffmpeg_progress_line(&l, &mut progress);
                    progress_manager.update_chapter_progress(&progress);
                }
            }
        }
        
        let status = cmd.wait()
            .with_context(|| format!("ffmpeg process failed for chapter: {}", chapter.title))?;
        
        if status.success() {
            progress_manager.complete_chapter(&chapter.title, &output_path.to_string_lossy(), duration_seconds);
            info!("Chapter {}/{} completed: {}", chapter_number, total_chapters, output_path.display());
        } else {
            error!("ffmpeg conversion failed for chapter: {}", chapter.title);
            progress_manager.emit_error(&format!("ffmpeg failed to convert chapter '{}'", chapter.title), Some(chapter_number));
            anyhow::bail!(
                "ffmpeg failed to convert chapter '{}'. Please check your input files and try again.",
                chapter.title
            );
        }
    }
    
    progress_manager.complete_all(true);
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

/// Run ffmpeg for a specific chapter with enhanced progress tracking
fn ffmpeg_chapter_with_progress(
    aaxc_file_path: PathBuf,
    audible_key: String,
    audible_iv: String,
    start_time: String,
    duration: String,
    file_name: String,
    codec: &str,
    _progress_manager: &ProgressManager,
    threads: &str,
) -> Result<Child> {
    let cmd = Command::new("ffmpeg")
        .args([
            "-audible_key",
            audible_key.as_str(),
            "-audible_iv",
            audible_iv.as_str(),
            "-i",
            aaxc_file_path
                .to_str()
                .context("Failed to convert input file path to string.")?,
            "-threads",
            threads,
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

    // Note: Progress parsing will be handled in the main thread
    // The progress manager will be updated by the calling function

    info!("ffmpeg process started for chapter");
    Ok(cmd)
}

/// Parse ffmpeg progress line and update progress struct
fn parse_ffmpeg_progress_line(line: &str, progress: &mut ConversionProgress) {
    // Parse time=HH:MM:SS.mmm
    if let Some(time_str) = line.strip_prefix("time=") {
        if let Ok(time_seconds) = parse_time_to_seconds(time_str.trim()) {
            progress.current_time = time_seconds;
        }
    }
    
    // Parse speed=N.Nx
    if let Some(speed_str) = line.strip_prefix("speed=") {
        if let Some(speed_value) = speed_str.strip_suffix('x') {
            if let Ok(speed) = speed_value.parse::<f64>() {
                progress.speed = speed;
            }
        }
    }
    
    // Parse bitrate=N
    if let Some(bitrate_str) = line.strip_prefix("bitrate=") {
        if let Ok(bitrate) = bitrate_str.trim().parse::<f64>() {
            progress.bitrate = bitrate;
        }
    }
    
    // Parse size=N
    if let Some(size_str) = line.strip_prefix("size=") {
        if let Ok(size) = size_str.trim().parse::<u64>() {
            progress.size = size;
        }
    }
    
    // Parse fps=N.N
    if let Some(fps_str) = line.strip_prefix("fps=") {
        if let Ok(fps) = fps_str.trim().parse::<f64>() {
            progress.fps = fps;
        }
    }
}

/// Parse time string (HH:MM:SS.mmm) to seconds
fn parse_time_to_seconds(time_str: &str) -> Result<f64, std::num::ParseFloatError> {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() == 3 {
        let hours: f64 = parts[0].parse()?;
        let minutes: f64 = parts[1].parse()?;
        let seconds: f64 = parts[2].parse()?;
        Ok(hours * 3600.0 + minutes * 60.0 + seconds)
    } else {
        // Fallback to direct parsing
        time_str.parse()
    }
}

/// Parse duration string (HH:MM:SS.mmm) to seconds
fn parse_duration_to_seconds(duration: &str) -> f64 {
    parse_time_to_seconds(duration).unwrap_or(0.0)
}


fn ffmpeg(
    aaxc_file_path: PathBuf,
    audible_key: String,
    audible_iv: String,
    duration: String,
    file_name: String,
    codec: &str,
    verbose: bool,
    machine_readable: bool,
    threads: &str,
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
            "-threads",
            threads,
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

        if machine_readable {
            // Machine-readable mode: output JSON progress events
            let mut progress = ConversionProgress::new(parse_duration_to_seconds(&duration));

            for line in stdout_reader.lines() {
                let l = line.context("Failed to read line from ffmpeg output.")?;
                parse_ffmpeg_progress_line(&l, &mut progress);
                
                let event = ProgressEvent::ChapterProgress {
                    chapter_number: 1,
                    total_chapters: 1,
                    chapter_title: "Single File".to_string(),
                    progress_percentage: progress.percentage(),
                    current_time: progress.current_time,
                    total_duration: progress.total_duration,
                    speed: progress.speed,
                    bitrate: progress.bitrate,
                    file_size: progress.size,
                    fps: progress.fps,
                    eta_seconds: progress.eta().map(|eta| eta.as_secs() as f64),
                };
                println!("{}", event.to_json());
            }
        } else {
            // Enhanced progress bar setup
            let pb = ProgressBar::new(100);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{bar:40.cyan/blue} {percent:>3}% [{elapsed_precise}] {msg}")
                    .unwrap()
                    .progress_chars("█▉▊▋▌▍▎▏  "),
            );
            pb.set_message("Starting conversion...");
            pb.enable_steady_tick(std::time::Duration::from_millis(100));

            let mut progress = ConversionProgress::new(parse_duration_to_seconds(&duration));

            for line in stdout_reader.lines() {
                let l = line.context("Failed to read line from ffmpeg output.")?;
                parse_ffmpeg_progress_line(&l, &mut progress);
                
                // Update progress bar
                let percentage = progress.percentage() as u64;
                pb.set_position(percentage);
                
                let eta_str = progress.eta()
                    .map(|eta| format!("ETA: {}", ProgressManager::format_duration(eta)))
                    .unwrap_or_else(|| "ETA: --:--:--".to_string());
                
                let speed_str = if progress.speed > 0.0 {
                    format!("Speed: {:.1}x", progress.speed)
                } else {
                    "Speed: --".to_string()
                };
                
                let bitrate_str = if progress.bitrate > 0.0 {
                    format!("Bitrate: {:.0} kbps", progress.bitrate / 1000.0)
                } else {
                    "Bitrate: --".to_string()
                };
                
                let size_str = if progress.size > 0 {
                    format!("Size: {}", ConversionProgress::format_size(progress.size))
                } else {
                    "Size: --".to_string()
                };

                let fps_str = if verbose && progress.fps > 0.0 {
                    format!("FPS: {:.1}", progress.fps)
                } else {
                    String::new()
                };

                let time_str = if verbose {
                    format!("Time: {}/{}", 
                        ConversionProgress::format_time(progress.current_time),
                        ConversionProgress::format_time(progress.total_duration))
                } else {
                    String::new()
                };

                let mut message_parts = vec![eta_str, speed_str, bitrate_str, size_str];
                if verbose {
                    if !fps_str.is_empty() {
                        message_parts.push(fps_str);
                    }
                    if !time_str.is_empty() {
                        message_parts.push(time_str);
                    }
                }

                pb.set_message(message_parts.join(" | "));

                // Log detailed progress in verbose mode
                if verbose {
                    info!("Progress: {:.1}% | Time: {}/{} | Speed: {:.1}x | Bitrate: {:.0} kbps | Size: {}", 
                        progress.percentage(),
                        ConversionProgress::format_time(progress.current_time),
                        ConversionProgress::format_time(progress.total_duration),
                        progress.speed,
                        progress.bitrate / 1000.0,
                        ConversionProgress::format_size(progress.size)
                    );
                }
            }
            pb.finish_with_message("Conversion complete");
        }
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
