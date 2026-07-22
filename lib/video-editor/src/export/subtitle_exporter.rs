use super::progress::{CancellationToken, ExportPhase, ExportProgress};
use crate::{
    Error, Result,
    filters::traits::SubtitleEntry,
    tracks::{manager::Manager, subtitle_track::SubtitleTrack, track::Track},
};
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

#[derive(Debug, Clone, derive_setters::Setters, derivative::Derivative)]
#[derivative(Default)]
#[setters(prefix = "with_")]
pub struct SubtitleExportConfig {
    #[derivative(Default(value = "PathBuf::from(\"subtitle\")"))]
    pub output_base_path: PathBuf,

    pub cancellation_token: Option<CancellationToken>,

    #[derivative(Default(value = "SubtitleFormat::Srt"))]
    pub format: SubtitleFormat,

    #[derivative(Default(value = "true"))]
    pub include_track_index: bool,

    pub suffix: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubtitleFormat {
    #[default]
    Srt,
    Vtt,
    Ass,
}

impl SubtitleFormat {
    pub fn extension(&self) -> &str {
        match self {
            Self::Srt => ".srt",
            Self::Vtt => ".vtt",
            Self::Ass => ".ass",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SubtitleExportResult {
    pub output_path: PathBuf,
    pub entry_count: usize,
    pub track_index: usize,
}

pub struct SubtitleExporter {
    manager: Arc<Manager>,
    config: SubtitleExportConfig,
}

impl SubtitleExporter {
    pub fn new(manager: Arc<Manager>, config: SubtitleExportConfig) -> Self {
        Self { manager, config }
    }

    fn check_cancelled(&self) -> Result<()> {
        if let Some(token) = &self.config.cancellation_token
            && token.is_cancelled()
        {
            return Err(Error::ExportCancelled);
        }
        Ok(())
    }

    pub fn export_track(&self, track_index: usize) -> Result<SubtitleExportResult> {
        let (subtitle_track, actual_index) = self.find_subtitle_track(track_index)?;
        let output_path = self.generate_output_path(actual_index);
        let entries = subtitle_track.get_subtitle_entries();

        match self.config.format {
            SubtitleFormat::Srt => self.export_srt(&output_path, &entries)?,
            SubtitleFormat::Vtt => self.export_vtt(&output_path, &entries)?,
            SubtitleFormat::Ass => self.export_ass(&output_path, &entries)?,
        }

        log::info!(
            "Exported {} subtitle entries to {}",
            entries.len(),
            output_path.display()
        );

        Ok(SubtitleExportResult {
            output_path,
            entry_count: entries.len(),
            track_index: actual_index,
        })
    }

    pub fn export_all_tracks(&self) -> Result<Vec<SubtitleExportResult>> {
        let mut results = Vec::new();

        for (idx, _track) in self.find_all_subtitle_tracks()?.iter().enumerate() {
            match self.export_track(idx) {
                Ok(result) => results.push(result),
                Err(e) => log::warn!("Failed to export subtitle track {}: {}", idx, e),
            }
        }

        Ok(results)
    }

    pub fn export_all_tracks_with_progress<F>(
        &self,
        mut progress_fn: F,
    ) -> Result<Vec<SubtitleExportResult>>
    where
        F: FnMut(ExportProgress),
    {
        let tracks = self.find_all_subtitle_tracks()?;
        let total_tracks = tracks.len();

        if total_tracks == 0 {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();

        for (idx, _track) in tracks.iter().enumerate() {
            self.check_cancelled()?;
            progress_fn(ExportProgress {
                current_position: Duration::from_secs_f64(idx as f64),
                total_duration: Duration::from_secs_f64(total_tracks as f64),
                frames_processed: idx as u64,
                total_frames: total_tracks as u64,
                phase: ExportPhase::EncodingVideo,
            });

            match self.export_track(idx) {
                Ok(result) => results.push(result),
                Err(Error::ExportCancelled) => return Err(Error::ExportCancelled),
                Err(e) => log::warn!("Failed to export subtitle track {}: {}", idx, e),
            }
        }

        progress_fn(ExportProgress {
            current_position: Duration::from_secs_f64(total_tracks as f64),
            total_duration: Duration::from_secs_f64(total_tracks as f64),
            frames_processed: total_tracks as u64,
            total_frames: total_tracks as u64,
            phase: ExportPhase::Complete,
        });

        Ok(results)
    }

    fn find_subtitle_track(&self, track_index: usize) -> Result<(&Arc<SubtitleTrack>, usize)> {
        let tracks = self.find_all_subtitle_tracks()?;
        let track = tracks
            .get(track_index)
            .ok_or_else(|| Error::IndexOutOfBounds(track_index, tracks.len()))?;
        Ok((track, track_index))
    }

    fn find_all_subtitle_tracks(&self) -> Result<Vec<&Arc<SubtitleTrack>>> {
        let mut tracks = Vec::new();

        for track in &self.manager.tracks {
            if let Track::Subtitle(subtitle_track) = track
                && !subtitle_track.hiding
            {
                tracks.push(subtitle_track);
            }
        }

        if tracks.is_empty() {
            return Err(Error::InvalidConfig("No subtitle tracks found".to_string()));
        }

        Ok(tracks)
    }

    fn generate_output_path(&self, track_index: usize) -> PathBuf {
        let mut path = self.config.output_base_path.clone();

        if self.config.include_track_index {
            let filename = format!(
                "{}_{}{}",
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("subtitle"),
                track_index,
                self.config.format.extension()
            );
            path.set_file_name(filename);
        } else {
            path.set_extension(self.config.format.extension().trim_start_matches('.'));
        }

        if let Some(suffix) = &self.config.suffix {
            let new_name = format!(
                "{}_{}{}",
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("subtitle"),
                suffix,
                self.config.format.extension()
            );
            path.set_file_name(new_name);
        }

        path
    }

    fn export_srt(&self, path: &Path, entries: &[SubtitleEntry]) -> Result<()> {
        let mut file = File::create(path).map_err(|e| {
            Error::IO(std::io::Error::new(
                e.kind(),
                format!("Failed to create {}: {}", path.display(), e),
            ))
        })?;

        for (idx, entry) in entries.iter().enumerate() {
            self.check_cancelled()?;
            writeln!(file, "{}", idx + 1)?;
            writeln!(
                file,
                "{} --> {}",
                format_timestamp_srt(entry.start),
                format_timestamp_srt(entry.end)
            )?;
            writeln!(file, "{}", entry.text)?;
            writeln!(file)?;
        }

        Ok(())
    }

    fn export_vtt(&self, path: &Path, entries: &[SubtitleEntry]) -> Result<()> {
        let mut file = File::create(path).map_err(|e| {
            Error::IO(std::io::Error::new(
                e.kind(),
                format!("Failed to create {}: {}", path.display(), e),
            ))
        })?;

        writeln!(file, "WEBVTT")?;
        writeln!(file)?;

        for entry in entries {
            self.check_cancelled()?;
            writeln!(
                file,
                "{} --> {}",
                format_timestamp_vtt(entry.start),
                format_timestamp_vtt(entry.end)
            )?;
            writeln!(file, "{}", entry.text)?;
            writeln!(file)?;
        }

        Ok(())
    }

    fn export_ass(&self, path: &Path, entries: &[SubtitleEntry]) -> Result<()> {
        let mut file = File::create(path).map_err(|e| {
            Error::IO(std::io::Error::new(
                e.kind(),
                format!("Failed to create {}: {}", path.display(), e),
            ))
        })?;

        // Write ASS header
        writeln!(file, "[Script Info]")?;
        writeln!(file, "ScriptType: v4.00+")?;
        writeln!(file, "WrapStyle: 0")?;
        writeln!(file, "PlayResX: 1920")?;
        writeln!(file, "PlayResY: 1080")?;
        writeln!(file)?;
        writeln!(file, "[V4+ Styles]")?;
        writeln!(
            file,
            "Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding"
        )?;
        writeln!(
            file,
            "Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1"
        )?;
        writeln!(file)?;
        writeln!(file, "[Events]")?;
        writeln!(
            file,
            "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text"
        )?;

        for entry in entries {
            self.check_cancelled()?;
            writeln!(
                file,
                "Dialogue: 0,{} {},Default,,0,0,0,,{}",
                format_timestamp_ass(entry.start),
                format_timestamp_ass(entry.end),
                entry.text.replace(',', "\\")
            )?;
        }

        Ok(())
    }
}

fn format_timestamp_srt(duration: Duration) -> String {
    let total_ms = duration.as_millis();
    let hours = total_ms / 3600000;
    let minutes = (total_ms % 3600000) / 60000;
    let seconds = (total_ms % 60000) / 1000;
    let ms = total_ms % 1000;
    format!("{:02}:{:02}:{:02},{:03}", hours, minutes, seconds, ms)
}

fn format_timestamp_vtt(duration: Duration) -> String {
    let total_ms = duration.as_millis();
    let hours = total_ms / 3600000;
    let minutes = (total_ms % 3600000) / 60000;
    let seconds = (total_ms % 60000) / 1000;
    let ms = total_ms % 1000;
    format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, seconds, ms)
}

fn format_timestamp_ass(duration: Duration) -> String {
    let total_cs = duration.as_secs_f64() * 100.0;
    let hours = (total_cs / 360000.0) as u32;
    let minutes = ((total_cs % 360000.0) / 6000.0) as u32;
    let seconds = ((total_cs % 6000.0) / 100.0) as u32;
    let cs = (total_cs % 100.0) as u32;
    format!("{}:{:02}:{:02}.{:02}", hours, minutes, seconds, cs)
}
