use crate::{Result, commands::ExportCommand, export::ProgressCallback};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportTaskStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy)]
pub struct ExportQueueStats {
    pub total: usize,
    pub pending: usize,
    pub completed: usize,
    pub failed: usize,
}

pub struct ExportTask {
    pub id: usize,
    pub name: String,
    pub command: ExportCommand,
    pub status: ExportTaskStatus,
    pub progress_callback: Option<ProgressCallback>,
    pub error: Option<String>,
}

impl ExportTask {
    pub fn new(id: usize, name: String, command: ExportCommand) -> Self {
        Self {
            id,
            name,
            command,
            status: ExportTaskStatus::Pending,
            progress_callback: None,
            error: None,
        }
    }

    pub fn with_progress_callback(mut self, cb: ProgressCallback) -> Self {
        self.progress_callback = Some(cb);
        self
    }

    pub fn update_status(&mut self, status: ExportTaskStatus) {
        self.status = status;
    }

    pub fn mark_failed(&mut self, error: String) {
        self.status = ExportTaskStatus::Failed;
        self.error = Some(error);
    }
}

pub struct ExportQueue {
    pending: VecDeque<ExportTask>,
    active: Option<ExportTask>,
    completed: Vec<ExportTask>,
    failed: Vec<ExportTask>,
    next_id: usize,
}

impl ExportQueue {
    pub fn new() -> Self {
        Self {
            pending: VecDeque::new(),
            active: None,
            completed: Vec::new(),
            failed: Vec::new(),
            next_id: 0,
        }
    }

    pub fn add(&mut self, name: String, command: ExportCommand) -> usize {
        let id = self.next_id;
        self.next_id += 1;

        let task = ExportTask::new(id, name, command);
        self.pending.push_back(task);

        log::debug!("Added export task {} to queue", id);
        id
    }

    pub fn len(&self) -> usize {
        self.pending.len()
            + self.active.is_some() as usize
            + self.completed.len()
            + self.failed.len()
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn completed_count(&self) -> usize {
        self.completed.len()
    }

    pub fn failed_count(&self) -> usize {
        self.failed.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty() && self.active.is_none()
    }

    pub fn is_complete(&self) -> bool {
        self.pending.is_empty() && self.active.is_none()
    }

    pub fn get_completed(&self) -> &[ExportTask] {
        &self.completed
    }

    pub fn get_failed(&self) -> &[ExportTask] {
        &self.failed
    }

    pub fn get_task(&self, id: usize) -> Option<&ExportTask> {
        self.pending.iter().find(|t| t.id == id).or_else(|| {
            self.active
                .as_ref()
                .and_then(|t| if t.id == id { Some(t) } else { None })
        })
    }

    pub fn process(&mut self) -> Result<()> {
        if let Some(task) = self.active.take() {
            match &task.status {
                ExportTaskStatus::Pending | ExportTaskStatus::Processing => {
                    self.active = Some(task);
                    return Ok(());
                }
                ExportTaskStatus::Completed => {
                    log::info!("Task {} completed: {}", task.id, task.name);
                    self.completed.push(task);
                }
                ExportTaskStatus::Failed => {
                    log::warn!("Task {} failed: {}", task.id, task.name);
                    self.failed.push(task);
                }
            }
        }

        if let Some(mut task) = self.pending.pop_front() {
            log::info!("Starting task {} of {}", task.id, task.name);
            task.update_status(ExportTaskStatus::Processing);

            match if let Some(progress_cb) = task.progress_callback.take() {
                task.command.execute_with_progress(progress_cb)
            } else {
                task.command.execute_simple()
            } {
                Ok(()) => {
                    task.update_status(ExportTaskStatus::Completed);
                    self.completed.push(task);
                }
                Err(e) => {
                    task.mark_failed(e.to_string());
                    self.failed.push(task);
                }
            }
        }

        Ok(())
    }

    pub fn process_all(&mut self) -> Result<()> {
        while !self.is_empty() {
            self.process()?;
        }

        Ok(())
    }

    pub fn clear_history(&mut self) {
        self.completed.clear();
        self.failed.clear();
    }

    pub fn clear_all(&mut self) {
        self.pending.clear();
        self.active = None;
        self.completed.clear();
        self.failed.clear();
        self.next_id = 0;
    }

    pub fn stats(&self) -> ExportQueueStats {
        ExportQueueStats {
            total: self.len(),
            pending: self.pending_count(),
            completed: self.completed_count(),
            failed: self.failed_count(),
        }
    }
}
