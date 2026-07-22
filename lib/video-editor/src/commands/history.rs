use super::{
    batch::BatchCommand,
    command::{AffectedSegments, Command},
};
use crate::{Error, Result, tracks::manager::Manager};

#[derive(derivative::Derivative)]
#[derivative(Default)]
pub struct HistoryManager {
    undo_stack: Vec<Box<dyn Command>>,
    redo_stack: Vec<Box<dyn Command>>,

    #[derivative(Default(value = "1000"))]
    max_history_size: usize,

    batch_depth: usize, // 记录嵌套批量命令的深度
    current_batch: Option<BatchCommand>,
}

pub struct UndoRedoResult {
    pub description: String,
    pub affected_segments: AffectedSegments,
}

pub struct ExecuteResult {
    pub affected_segments: AffectedSegments,
}

impl HistoryManager {
    pub fn new() -> Self {
        Self::default()
    }

    // Set maximum history size (0 = unlimited)
    // When the limit is reached, the oldest commands are removed first.
    pub fn with_max_history(mut self, size: usize) -> Self {
        self.max_history_size = size;
        self
    }

    // Execute a command and add it to history
    // If the command executes successfully, it's added to the undo stack
    // and the redo stack is cleared (since a new command invalidates redo history).
    pub fn execute(
        &mut self,
        manager: &mut Manager,
        mut command: Box<dyn Command>,
    ) -> Result<ExecuteResult> {
        command.execute(manager)?;
        let affected_segments = command.affected_segments_after_execute();
        self.redo_stack.clear();

        if let Some(ref mut batch) = self.current_batch {
            batch.add_command(command);
            return Ok(ExecuteResult { affected_segments });
        }

        if self.max_history_size > 0 && self.undo_stack.len() >= self.max_history_size {
            self.undo_stack.remove(0);
        }

        self.undo_stack.push(command);
        Ok(ExecuteResult { affected_segments })
    }

    pub fn undo(&mut self, manager: &mut Manager) -> Result<UndoRedoResult> {
        let mut command = self
            .undo_stack
            .pop()
            .ok_or_else(|| Error::CannotUndo("No commands to undo".into()))?;

        let description = command.describe();
        command.undo(manager)?;
        let affected_segments = command.affected_segments_after_undo();
        self.redo_stack.push(command);

        Ok(UndoRedoResult {
            description,
            affected_segments,
        })
    }

    pub fn redo(&mut self, manager: &mut Manager) -> Result<UndoRedoResult> {
        let mut command = self
            .redo_stack
            .pop()
            .ok_or_else(|| Error::CannotRedo("No commands to redo".into()))?;

        let description = command.describe();
        command.execute(manager)?;
        let affected_segments = command.affected_segments_after_execute();
        self.undo_stack.push(command);

        Ok(UndoRedoResult {
            description,
            affected_segments,
        })
    }

    /// Start a batch operation (multiple commands treated as one)
    ///
    /// Batches can be nested. Only when the outermost batch ends is the
    /// combined command added to history.
    ///
    /// # Example
    ///
    /// ```
    /// use video_editor::commands::HistoryManager;
    /// use video_editor::commands::batch::BatchCommand;
    ///
    /// let mut history = HistoryManager::new();
    ///
    /// // Begin a batch operation
    /// history.begin_batch("Complex Edit".to_string());
    ///
    /// // In a real scenario, you would add commands through HistoryManager::execute
    /// // The batch will automatically collect commands until end_batch is called
    ///
    /// // End the batch operation (the batch command is automatically pushed to undo stack)
    /// history.end_batch().unwrap();
    /// ```
    pub fn begin_batch(&mut self, name: String) {
        self.batch_depth += 1;
        if self.current_batch.is_none() {
            self.current_batch = Some(BatchCommand::new(name));
        }
    }

    /// End a batch operation and push the combined command to undo stack.
    ///
    /// Returns error if called without a matching `begin_batch`.
    /// For nested batches, only the outermost `end_batch` pushes the command.
    pub fn end_batch(&mut self) -> Result<()> {
        if self.batch_depth == 0 {
            return Err(Error::InvalidConfig("Should call begin_batch first".into()));
        }

        self.batch_depth = self.batch_depth.saturating_sub(1);

        if self.batch_depth == 0
            && let Some(mut batch) = self.current_batch.take()
        {
            batch.mark_all_executed();
            self.push_command(Box::new(batch));
        }

        Ok(())
    }

    // Push a command directly to the undo stack.
    // This is primarily used internally by `end_batch`, but can also be used
    // for externally constructed batch commands in special scenarios.
    pub fn push_command(&mut self, command: Box<dyn Command>) {
        if self.max_history_size > 0 && self.undo_stack.len() >= self.max_history_size {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(command);
        self.redo_stack.clear();
    }

    pub fn reset_batch(&mut self) {
        self.batch_depth = 0;
        self.current_batch = None;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_history(&self) -> Vec<String> {
        self.undo_stack.iter().map(|c| c.describe()).collect()
    }

    pub fn redo_history(&self) -> Vec<String> {
        self.redo_stack.iter().map(|c| c.describe()).collect()
    }

    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}
