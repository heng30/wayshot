use crate::{
    Error, Result,
    commands::Command,
    export::{ExportResult, Mp4Exporter, config::Mp4ExportConfig, progress::ProgressCallback},
    tracks::manager::Manager,
};
use rayon::prelude::*;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct ExportCommand {
    config: Mp4ExportConfig,
    manager: Arc<Manager>,
    result: Arc<Mutex<Option<ExportResult>>>,
}

impl ExportCommand {
    pub fn new(manager: Arc<Manager>, config: Mp4ExportConfig) -> Self {
        Self {
            config,
            manager,
            result: Arc::new(Mutex::new(None)),
        }
    }

    pub fn result(&self) -> Option<ExportResult> {
        self.result.lock().unwrap().clone()
    }

    fn execute_export_with_callback(
        &self,
        progress_callback: Option<ProgressCallback>,
    ) -> Result<ExportResult> {
        let exporter = Mp4Exporter::new_arc(self.manager.clone(), self.config.clone());

        let result = if let Some(callback) = progress_callback {
            exporter.export_with_progress(callback)?
        } else {
            exporter.export()?
        };

        *self.result.lock().unwrap() = Some(result.clone());

        Ok(result)
    }

    pub fn execute_with_progress(&mut self, callback: ProgressCallback) -> Result<()> {
        self.execute_export_with_callback(Some(callback))?;
        Ok(())
    }

    pub fn execute_simple(&mut self) -> Result<()> {
        self.execute_export_with_callback(None)?;
        Ok(())
    }
}

impl Command for ExportCommand {
    fn describe(&self) -> String {
        format!("Export to {}", self.config.output_path.display())
    }

    fn execute(&mut self, _manager: &mut Manager) -> Result<()> {
        self.execute_simple()?;
        Ok(())
    }

    fn undo(&mut self, _manager: &mut Manager) -> Result<()> {
        let result_guard = self.result.lock().unwrap();
        let result = result_guard
            .as_ref()
            .ok_or_else(|| Error::CannotUndo("No export result".to_string()))?;
        let output_path = &result.output_path;

        if output_path.exists() {
            std::fs::remove_file(output_path).map_err(|e| {
                Error::IO(std::io::Error::other(format!(
                    "Failed to remove exported file: {}",
                    e
                )))
            })?;
        }

        log::info!("Undid export: deleted {}", output_path.display());
        Ok(())
    }
}

#[derive(Debug)]
pub struct BatchExportCommand {
    parallel: bool,
    commands: Vec<ExportCommand>,
    results: Arc<Mutex<Vec<Option<ExportResult>>>>,
}

impl BatchExportCommand {
    pub fn new(parallel: bool) -> Self {
        Self {
            parallel,
            commands: Vec::new(),
            results: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add(&mut self, command: ExportCommand) -> &mut Self {
        self.commands.push(command);
        self.results.lock().unwrap().push(None);
        self
    }

    pub fn results(&self) -> Vec<Option<ExportResult>> {
        self.results.lock().unwrap().clone()
    }

    pub fn successful_results(&self) -> Vec<ExportResult> {
        self.results
            .lock()
            .unwrap()
            .iter()
            .filter_map(|opt| opt.clone())
            .collect()
    }

    fn execute_all(&self) -> Result<Vec<Option<ExportResult>>> {
        let mut results = Vec::new();

        if self.parallel {
            results = self
                .commands
                .par_iter()
                .enumerate()
                .map(|(i, cmd)| {
                    log::info!(
                        "Starting parallel export {} of {}",
                        i + 1,
                        self.commands.len()
                    );

                    match cmd.execute_export_with_callback(None) {
                        Ok(result) => {
                            log::info!("Export {} completed: {:?}", i + 1, result.output_path);
                            Some(result)
                        }
                        Err(e) => {
                            log::error!("Export {} failed: {}", i + 1, e);
                            None
                        }
                    }
                })
                .collect();
        } else {
            for (i, cmd) in self.commands.iter().enumerate() {
                log::info!("Starting export {} of {}", i + 1, self.commands.len());

                match cmd.execute_export_with_callback(None) {
                    Ok(result) => {
                        log::info!("Export {} completed: {:?}", i + 1, result.output_path);
                        results.push(Some(result));
                    }
                    Err(e) => {
                        log::error!("Export {} failed: {}", i + 1, e);
                        results.push(None);
                    }
                }
            }
        }

        Ok(results)
    }
}

impl Command for BatchExportCommand {
    fn describe(&self) -> String {
        format!("Batch export ({} items)", self.commands.len())
    }

    fn execute(&mut self, _manager: &mut Manager) -> Result<()> {
        let results = self.execute_all()?;
        *self.results.lock().unwrap() = results;

        Ok(())
    }

    fn undo(&mut self, _manager: &mut Manager) -> Result<()> {
        for (i, cmd) in self.commands.iter().enumerate() {
            if let Some(result) = cmd.result()
                && result.output_path.exists()
            {
                std::fs::remove_file(&result.output_path).map_err(|e| {
                    Error::IO(std::io::Error::other(format!(
                        "Failed to remove exported file {}: {e}",
                        i + 1,
                    )))
                })?;

                log::info!(
                    "Undid export {}: deleted {}",
                    i + 1,
                    result.output_path.display()
                );
            }
        }

        Ok(())
    }
}
