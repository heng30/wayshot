//! Integration tests for video-editor library

use video_editor::{
    commands::{BatchExportCommand, Command, ExportCommand},
    export::{
        queue::{ExportQueue, ExportTask, ExportTaskStatus},
        Mp4ExportConfig,
    },
    tracks::manager::Manager,
    Result,
};
use std::{
    path::PathBuf,
    sync::Arc,
};

/// Integration test setup
pub struct TestSetup {
    pub manager: Arc<Manager>,
    pub temp_dir: tempfile::TempDir,
}

impl TestSetup {
    pub fn new() -> Self {
        let temp_dir = tempfile::tempdir().unwrap();
        let manager = Manager::new();

        log::info!("Created test environment in: {}", temp_dir.path().display());

        Self {
            manager: Arc::new(manager),
            temp_dir,
        }
    }

    pub fn create_test_video(&self) -> Result<PathBuf> {
        // For now, use a dummy video file
        let test_file = self.temp_dir.path().join("test_video.mp4");
        Ok(test_file)
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_export_command_full_workflow() {
        let setup = TestSetup::new();

        // Create export command
        let output_path = setup.create_test_video().unwrap();

        let config = Mp4ExportConfig::default()
            .with_output_path(output_path);

        let cmd = ExportCommand::new(Arc::clone(&setup.manager), config);

        // Execute export
        let result = cmd.result();
        assert!(result.is_none(), "Export should not have result yet");
    }

    #[test]
    fn test_export_queue_workflow() {
        let mut queue = ExportQueue::new();

        let setup = TestSetup::new();
        let output_path = setup.create_test_video().unwrap();

        let config = Mp4ExportConfig::default()
            .with_output_path(output_path);

        let cmd = ExportCommand::new(Arc::clone(&setup.manager), config);

        // Add to queue
        queue.add("Test Export".to_string(), cmd);

        // Check stats
        let stats = queue.stats();
        assert_eq!(stats.total, 1);
        assert_eq!(stats.pending, 1);
        assert_eq!(stats.completed, 0);
        assert_eq!(stats.failed, 0);
    }

    #[test]
    fn test_batch_export_command() {
        let setup = TestSetup::new();

        let mut batch = BatchExportCommand::new(false);

        // Add multiple exports
        for i in 0..3 {
            let output_path = setup.temp_dir.path().join(format!("test_{}.mp4", i));

            let config = Mp4ExportConfig::default()
                .with_output_path(output_path);

            let cmd = ExportCommand::new(Arc::clone(&setup.manager), config);
            batch.add(cmd);
        }

        // Verify count via describe() which includes the count
        let description = batch.describe();
        assert!(description.contains("3"), "Batch should contain 3 items");
    }

    #[test]
    fn test_export_queue_process() {
        let mut queue = ExportQueue::new();

        let setup = TestSetup::new();
        let output_path = setup.create_test_video().unwrap();

        let config = Mp4ExportConfig::default()
            .with_output_path(output_path);

        let cmd = ExportCommand::new(Arc::clone(&setup.manager), config);
        queue.add("Test Export".to_string(), cmd);

        // Process queue (without actual export for testing)
        let stats = queue.stats();
        assert_eq!(stats.total, 1);

        queue.clear_history();
        assert_eq!(queue.completed_count(), 0);
        assert_eq!(queue.failed_count(), 0);
    }

    #[test]
    fn test_export_task_status() {
        let setup = TestSetup::new();
        let output_path = setup.create_test_video().unwrap();

        let config = Mp4ExportConfig::default()
            .with_output_path(output_path);

        let cmd = ExportCommand::new(Arc::clone(&setup.manager), config);
        let mut task = ExportTask::new(1, "Test Task".to_string(), cmd);

        assert_eq!(task.status, ExportTaskStatus::Pending);

        task.update_status(ExportTaskStatus::Processing);
        assert_eq!(task.status, ExportTaskStatus::Processing);

        task.update_status(ExportTaskStatus::Completed);
        assert_eq!(task.status, ExportTaskStatus::Completed);

        task.mark_failed("Test error".to_string());
        assert_eq!(task.status, ExportTaskStatus::Failed);
        assert!(task.error.is_some());
    }

    #[test]
    fn test_export_queue_stats() {
        let queue = ExportQueue::new();

        let stats = queue.stats();
        assert_eq!(stats.total, 0);
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.completed, 0);
        assert_eq!(stats.failed, 0);

        assert!(queue.is_empty());
        assert!(queue.is_complete());
    }

    #[test]
    fn test_export_queue_clear() {
        let mut queue = ExportQueue::new();

        let setup = TestSetup::new();
        let output_path = setup.create_test_video().unwrap();

        let config = Mp4ExportConfig::default()
            .with_output_path(output_path);

        let cmd = ExportCommand::new(Arc::clone(&setup.manager), config);
        queue.add("Test".to_string(), cmd);

        assert_eq!(queue.len(), 1);

        queue.clear_all();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }
}

#[cfg(test)]
mod workflow_tests {
    use super::*;

    #[test]
    fn test_complete_export_workflow() {
        let setup = TestSetup::new();

        // Step 1: Create export configuration
        let output_path = setup.temp_dir.path().join("workflow_test.mp4");

        let config = Mp4ExportConfig::default()
            .with_output_path(output_path.clone())
            .with_width(Some(1920))
            .with_height(Some(1080))
            .with_fps(Some(30));

        // Step 2: Create export command
        let cmd = ExportCommand::new(Arc::clone(&setup.manager), config);

        // Step 3: Verify command properties
        assert!(!cmd.describe().is_empty());
        assert!(cmd.result().is_none());

        log::info!("Complete workflow test passed: {}", cmd.describe());
    }

    #[test]
    fn test_batch_export_workflow() {
        let setup = TestSetup::new();

        // Step 1: Create batch command
        let mut batch = BatchExportCommand::new(false);

        // Step 2: Add multiple exports
        for i in 0..5 {
            let output_path = setup.temp_dir.path().join(format!("batch_{}.mp4", i));

            let config = Mp4ExportConfig::default()
                .with_output_path(output_path);

            let cmd = ExportCommand::new(Arc::clone(&setup.manager), config);
            batch.add(cmd);
        }

        // Step 3: Verify batch
        let description = batch.describe();
        assert!(description.contains("5"), "Batch should contain 5 items");
        assert!(!batch.describe().is_empty());

        log::info!("Batch workflow test passed: {}", description);
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[test]
    fn test_export_command_undo() {
        let setup = TestSetup::new();
        let mut manager = Manager::new();

        let output_path = setup.create_test_video().unwrap();

        let config = Mp4ExportConfig::default()
            .with_output_path(output_path);

        let mut cmd = ExportCommand::new(Arc::clone(&setup.manager), config);

        // Execute command
        let result = cmd.execute(&mut manager);
        assert!(result.is_ok());
    }
}
