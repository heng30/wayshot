use super::command::{AffectedSegment, AffectedSegments, Command};
use crate::{Result, tracks::manager::Manager};

// 批量命令 - 将多个命令组合成一个原子操作
// 批量命令用于将多个编辑操作作为一个整体来执行和撤销。
// 如果批量命令中的任何一个子命令执行失败，所有已执行的子命令都会被撤销。
pub struct BatchCommand {
    name: String,
    commands: Vec<Box<dyn Command>>,
    executed_indices: Vec<usize>,

    // Extra affected segments to add after collecting from sub-commands
    // Used when the final position of a segment is known only after all commands are added
    extra_affected_segments: AffectedSegments,
}

impl BatchCommand {
    pub fn new(name: String) -> Self {
        Self {
            name,
            commands: Vec::new(),
            executed_indices: Vec::new(),
            extra_affected_segments: AffectedSegments::new(),
        }
    }

    pub fn add_command(&mut self, command: Box<dyn Command>) {
        self.commands.push(command);
    }

    // Mark all commands as executed.
    // Used in batch mode when commands are executed individually before being added to the batch.
    pub fn mark_all_executed(&mut self) {
        self.executed_indices = (0..self.commands.len()).collect();
    }

    pub fn add_extra_affected_segment(&mut self, segment: AffectedSegment) {
        self.extra_affected_segments.segments.push(segment);
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

impl Command for BatchCommand {
    fn execute(&mut self, manager: &mut Manager) -> Result<()> {
        self.executed_indices.clear();

        for (index, command) in self.commands.iter_mut().enumerate() {
            if let Err(e) = command.execute(manager) {
                // 如果执行失败，撤销已成功的命令
                for executed_index in self.executed_indices.iter().rev() {
                    _ = self.commands[*executed_index].undo(manager);
                }
                return Err(e);
            }
            self.executed_indices.push(index);
        }

        Ok(())
    }

    fn undo(&mut self, manager: &mut Manager) -> Result<()> {
        for index in self.executed_indices.iter().rev() {
            self.commands[*index].undo(manager)?;
        }
        Ok(())
    }

    fn describe(&self) -> String {
        format!("Batch operation: {}", self.name)
    }

    fn can_undo(&self) -> bool {
        self.commands.iter().all(|c| c.can_undo())
    }

    fn affected_segments_after_execute(&self) -> AffectedSegments {
        let mut result = AffectedSegments::new();
        for command in &self.commands {
            result.merge(command.affected_segments_after_execute());
        }
        result.merge(self.extra_affected_segments.clone());
        result
    }

    fn affected_segments_after_undo(&self) -> AffectedSegments {
        let mut result = AffectedSegments::new();
        for command in &self.commands {
            result.merge(command.affected_segments_after_undo());
        }
        result.merge(self.extra_affected_segments.clone());
        result
    }
}
