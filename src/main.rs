use clap::Parser;
use anyhow::{Result, Context, bail};
use std::path::PathBuf;
use dialoguer::{Select, Input};
use std::process::Command;
use which::which;
use std::collections::BTreeMap;
use colored::Colorize;

#[derive(Parser)]
#[command(
    name = "video-downloader",
    about = "A versatile media downloader supporting YouTube and other platforms",
    version
)]
struct Cli {
    /// Optional URL to download directly
    #[arg(short, long)]
    url: Option<String>,

    /// Output directory for downloaded files (optional, will prompt if not provided)
    #[arg(short, long)]
    output_dir: Option<PathBuf>,
    
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

enum DownloadType {
    Video,
    Audio,
}

#[derive(Clone)]
struct FormatOption {
    id: String,
    format_description: String,
    resolution: Option<u32>,
    is_video: bool,
    is_audio: bool,
    extension: String,
    filesize: Option<String>,
}

impl FormatOption {
    fn parse_format_line(line: &str) -> Option<Self> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 {
            return None;
        }

        let id = parts[0].to_string();
        let extension = parts[1].to_string();
        
        // Parse resolution
        let resolution = parts.iter()
            .find(|p| p.contains('x'))
            .and_then(|res| res.split('x').nth(1))
            .and_then(|height| height.parse().ok());

        // Determine if it's video and/or audio
        let is_video = !line.contains("audio only");
        let is_audio = line.contains("audio only");
        
        // Get filesize if available
        let filesize = if let Some(size_pos) = parts.iter().position(|&p| p == "MiB" || p == "KiB" || p == "GiB") {
            if size_pos > 0 {
                // Handle both exact and approximate sizes
                let size = parts[size_pos - 1].trim_start_matches('~').trim_start_matches("≈");
                Some(format!("{} {}", size, parts[size_pos]))
            } else {
                None
            }
        } else {
            None
        };

        // Get format description (exclude size info if present)
        let format_description = if let Some(size_pos) = parts.iter().position(|&p| p == "MiB" || p == "KiB" || p == "GiB") {
            parts[2..size_pos-1].join(" ")
        } else {
            parts[2..].join(" ")
        };

        Some(Self {
            id,
            format_description,
            resolution,
            is_video,
            is_audio,
            extension,
            filesize,
        })
    }
}

async fn ensure_yt_dlp() -> Result<()> {
    if which("yt-dlp").is_err() {
        println!("{}", "yt-dlp is not installed. Please install it first:".bold().red());
        println!("For Ubuntu/Debian: sudo apt install yt-dlp");
        println!("For other systems, visit: https://github.com/yt-dlp/yt-dlp#installation");
        bail!("yt-dlp not found");
    }
    Ok(())
}

fn parse_available_formats(formats_str: &str) -> Vec<FormatOption> {
    let mut formats = Vec::new();
    
    for line in formats_str.lines() {
        // Skip header lines
        if line.starts_with("ID") || line.starts_with("[info]") || line.trim().is_empty() {
            continue;
        }
        
        if let Some(format) = FormatOption::parse_format_line(line) {
            formats.push(format);
        }
    }
    
    formats
}

fn get_download_directory(cli_dir: Option<PathBuf>) -> Result<PathBuf> {
    match cli_dir {
        Some(dir) => Ok(dir),
        None => {
            // Default directory options
            let options = vec![
                "Current directory (./)".to_string(),
                "Downloads directory (~/Downloads)".to_string(),
                "Documents directory (~/Documents)".to_string(),
                "Videos directory (~/Videos)".to_string(),
                "Custom path (enter manually)".to_string(),
            ];
            
            let selection = Select::new()
                .with_prompt("Select download directory")
                .items(&options)
                .default(0)
                .interact()?;
                
            match selection {
                0 => Ok(PathBuf::from(".")),
                1 => Ok(dirs::download_dir().unwrap_or_else(|| PathBuf::from("./Downloads"))),
                2 => Ok(dirs::document_dir().unwrap_or_else(|| PathBuf::from("./Documents"))),
                3 => Ok(dirs::video_dir().unwrap_or_else(|| PathBuf::from("./Videos"))),
                4 => {
                    let path_str: String = Input::new()
                        .with_prompt("Enter custom download path")
                        .default("./downloads".into())
                        .interact()?;
                    Ok(PathBuf::from(path_str))
                },
                _ => unreachable!(),
            }
        }
    }
}

async fn download_media(url: &str, output_dir: &PathBuf, verbose: bool) -> Result<()> {
    ensure_yt_dlp().await?;

    // First, list available formats
    let mut list_formats = Command::new("yt-dlp");
    list_formats
        .arg(url)
        .arg("-F")
        .arg("--no-check-certificates")
        .arg("--force-ipv4");

    println!("{}", "Checking available formats...".bold().green());
    let formats = list_formats.output().context("Failed to list formats")?;
    let formats_str = String::from_utf8_lossy(&formats.stdout);
    
    if verbose {
        // Print raw format information in verbose mode
        println!("\n{}", "Available formats (raw):".bold().cyan());
        println!("{}", formats_str);
    }
    
    // Parse available formats
    let parsed_formats = parse_available_formats(&formats_str);
    
    // Group video formats by resolution for display
    let mut video_resolutions: BTreeMap<Option<u32>, Vec<&FormatOption>> = BTreeMap::new();
    let mut audio_formats: Vec<&FormatOption> = Vec::new();
    
    for format in &parsed_formats {
        if format.is_video && !format.is_audio {
            video_resolutions.entry(format.resolution).or_default().push(format);
        } else if format.is_audio && !format.is_video {
            audio_formats.push(format);
        }
    }
    
    // First ask if user wants video or audio
    let download_type = Select::new()
        .with_prompt("Select download type")
        .items(&["Video", "Audio"])
        .default(0)
        .interact()?;

    let download_type = match download_type {
        0 => DownloadType::Video,
        _ => DownloadType::Audio,
    };
    
    let output_template = match download_type {
        DownloadType::Video => format!("{}/%(title)s_%(height)sp.%(ext)s", output_dir.display()),
        DownloadType::Audio => format!("{}/%(title)s.%(ext)s", output_dir.display()),
    };
    
    match download_type {
        DownloadType::Video => {
            println!("\n{}", "Available Video Resolutions (MP4 only):".bold().cyan());
            
            // First, get the best audio format (prefer m4a for mp4 compatibility)
            let best_audio = audio_formats.iter()
                .find(|f| f.extension == "m4a")
                .or_else(|| audio_formats.first())
                .unwrap_or_else(|| panic!("No audio formats found"));

            // Collect only MP4 video formats by resolution
            let mut mp4_formats: Vec<(u32, FormatOption)> = video_resolutions.iter()
                .filter_map(|(res, formats)| {
                    res.map(|resolution| {
                        // Find best MP4 format for this resolution
                        let best_format = formats.iter()
                            .filter(|f| f.is_video && f.extension == "mp4")
                            .max_by_key(|f| {
                                // Prefer formats with higher bitrate (typically better quality)
                                f.format_description
                                    .split_whitespace()
                                    .find(|w| w.ends_with('k'))
                                    .and_then(|w| w.trim_end_matches('k').parse::<u32>().ok())
                                    .unwrap_or(0)
                            })
                            .map(|f| (*f).clone());
                        (resolution, best_format)
                    })
                })
                .filter_map(|(res, fmt)| fmt.map(|f| (res, f)))
                .collect();

            // Sort by resolution (highest first) and take top 5
            mp4_formats.sort_by(|(res_a, _), (res_b, _)| res_b.cmp(res_a));
            mp4_formats.truncate(5);

            if mp4_formats.is_empty() {
                println!("{}", "No MP4 formats found.".bold().red());
                bail!("No suitable MP4 formats found");
            }

            // Create quality options
            let mut quality_options = Vec::new();
            
            // Display available resolutions
            println!("\nSelect video quality (will be combined with best audio):");
            for (i, (resolution, format)) in mp4_formats.iter().enumerate() {
                let size_info = format.filesize.as_ref()
                    .map(|s| format!(" (~{})", s))
                    .unwrap_or_default();
                
                let quality_str = format!("{}p MP4{}", resolution, size_info);
                quality_options.push((quality_str.clone(), format.id.clone(), *resolution));
                
                println!("  {}. {} ({})", 
                    (i + 1).to_string().bold(), 
                    quality_str.bold().green(),
                    format.format_description.bright_black()
                );
            }
            
            // Let user select quality
            let selected_idx = Select::new()
                .with_prompt("Select video quality")
                .items(&quality_options.iter().map(|(q, _, _)| q.clone()).collect::<Vec<_>>())
                .default(0)
                .interact()?;
                
            // Get selected format
            let (quality_str, video_id, _) = &quality_options[selected_idx];
            
            // Combine with best audio
            let format_arg = format!("{}+{}", video_id, best_audio.id);
            
            println!("\n{}", "Starting download...".bold().green());
            println!("Quality: {}", quality_str.bold());
            println!("Video format: {} ({})", video_id.bold().yellow(), "MP4".bright_blue());
            println!("Audio format: {} ({})", best_audio.id.bold().yellow(), best_audio.format_description);
            println!("Download location: {}", output_dir.display().to_string().bold());

            // Configure download command
            let mut command = Command::new("yt-dlp");
            command
                .arg(url)
                .arg("-f").arg(&format_arg)
                .arg("-o").arg(&output_template)
                .arg("--progress")
                .arg("--no-check-certificates")
                .arg("--force-ipv4")
                .arg("--geo-bypass")
                .arg("--no-playlist")
                .arg("--merge-output-format").arg("mp4")
                .arg("--prefer-ffmpeg");
                
            if verbose {
                println!("\n{}", "Running command:".bold().cyan());
                println!("{:?}", command);
            }
            
            let status = command.status().context("Failed to execute yt-dlp")?;
            
            if !status.success() {
                println!("{}", "Download failed with primary format. Retrying with alternative method...".bold().yellow());
                
                // Fallback with simpler options
                let mut retry_command = Command::new("yt-dlp");
                retry_command
                    .arg(url)
                    .arg("-f").arg("bestvideo+bestaudio/best")
                    .arg("-o").arg(&output_template)
                    .arg("--force-ipv4")
                    .arg("--no-check-certificates")
                    .arg("--merge-output-format").arg("mp4")
                    .arg("--prefer-ffmpeg");
                
                let retry_status = retry_command.status().context("Failed to execute retry download")?;
                
                if !retry_status.success() {
                    bail!("Download failed after retry. Please try a different format or URL.");
                }
            }
            
            println!("{}", "Download completed successfully!".bold().green());
            println!("File saved to: {}", output_dir.display().to_string().bold());
        },
        DownloadType::Audio => {
            println!("\n{}", "Available Audio Formats:".bold().cyan());
            
            // Show only top 5 audio formats (sort by likely quality)
            let top_audio_formats: Vec<&FormatOption> = audio_formats.iter()
                .take(5)
                .copied()
                .collect();
            
            if top_audio_formats.len() < audio_formats.len() {
                println!("{}", "Showing only top 5 audio formats. Use verbose mode (-v) to see all.".yellow());
            }
            
            let mut audio_options = Vec::new();
            for (i, format) in top_audio_formats.iter().enumerate() {
                let size_info = format.filesize.as_ref()
                    .map(|s| format!(" ({})", s))
                    .unwrap_or_default();
                    
                let option_str = format!("{} - {}{}", format.id, format.extension, size_info);
                audio_options.push(option_str);
                
                println!("  {}. {}", i+1, format.format_description);
            }
            
            // Add option for best audio
            audio_options.push("Best audio (automatic selection)".to_string());
            audio_options.push("Custom format (enter format ID directly)".to_string());
            
            let selected_idx = Select::new()
                .with_prompt("Select audio format")
                .items(&audio_options)
                .default(audio_options.len() - 2) // Default to "Best audio"
                .interact()?;
                
            let format_arg = if selected_idx < top_audio_formats.len() {
                // User selected a specific audio format
                let format = top_audio_formats[selected_idx];
                println!("Selected audio format: {} ({})", 
                    format.id.bold().green(),
                    format.format_description);
                format.id.clone()
            } else if selected_idx == top_audio_formats.len() {
                // Best audio option
                println!("Selected: {}", "Best audio (automatic)".bold().green());
                "bestaudio".to_string()
            } else {
                // Custom format
                Input::<String>::new()
                    .with_prompt("Enter format ID or format specification (e.g. '140' or 'bestaudio')")
                    .default("bestaudio".into())
                    .interact()?
            };
            
            println!("\n{}", "Starting audio download...".bold().green());
            println!("Format specification: {}", format_arg.bold());
            println!("Download location: {}", output_dir.display().to_string().bold());
            
            let mut command = Command::new("yt-dlp");
            command
                .arg(url)
                .arg("-f").arg(&format_arg)
                .arg("-o").arg(&output_template)
                .arg("--progress")
                .arg("--no-check-certificates")
                .arg("--force-ipv4")
                .arg("--geo-bypass")
                .arg("--no-playlist")
                .arg("-x") // Extract audio
                .arg("--audio-format").arg("mp3") // Convert to mp3
                .arg("--prefer-ffmpeg");
                
            if verbose {
                println!("\n{}", "Running command:".bold().cyan());
                println!("{:?}", command);
            }
            
            let status = command.status().context("Failed to execute yt-dlp")?;
            
            if !status.success() {
                println!("{}", "Download failed with primary format. Retrying with alternative method...".bold().yellow());
                
                // Fallback with simpler options
                let mut retry_command = Command::new("yt-dlp");
                retry_command
                    .arg(url)
                    .arg("-f").arg("bestaudio")
                    .arg("-o").arg(&output_template)
                    .arg("--force-ipv4")
                    .arg("--no-check-certificates")
                    .arg("-x")
                    .arg("--audio-format").arg("mp3")
                    .arg("--prefer-ffmpeg");
                
                let retry_status = retry_command.status().context("Failed to execute retry download")?;
                
                if !retry_status.success() {
                    bail!("Audio download failed after retry. Please try a different format or URL.");
                }
            }
            
            println!("{}", "Audio download completed successfully!".bold().green());
            println!("File saved to: {}", output_dir.display().to_string().bold());
        }
    }
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    println!("{}", "YouTube Video Downloader".bold().blue().underline());

    // Get URL either from command line or user input
    let url = if let Some(url) = cli.url.clone() {
        url
    } else {
        Input::<String>::new()
            .with_prompt("Enter the URL to download")
            .interact()?
    };
    
    // Get download directory (from CLI or manual selection)
    let output_dir = get_download_directory(cli.output_dir)?;

    // Create output directory if it doesn't exist
    tokio::fs::create_dir_all(&output_dir).await
        .with_context(|| format!("Failed to create output directory: {}", output_dir.display()))?;
    
    println!("Download directory: {}", output_dir.display().to_string().bold());

    // Start the download process
    download_media(&url, &output_dir, cli.verbose).await?;

    Ok(())
}