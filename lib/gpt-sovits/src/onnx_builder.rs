use {
    crate::error::GSVError,
    ort::{
        execution_providers::CPUExecutionProvider,
        session::{Session, builder::GraphOptimizationLevel},
    },
    std::path::Path,
};

const INTRA_THREADS: usize = 8;

pub fn create_onnx_cpu_session<P>(path: P) -> Result<Session, GSVError>
where
    P: AsRef<Path>,
{
    Ok(Session::builder()?
        .with_execution_providers([CPUExecutionProvider::default()
            .with_arena_allocator(true)
            .build()])?
        .with_optimization_level(GraphOptimizationLevel::Level3)?
        .with_intra_threads(INTRA_THREADS)?
        .with_prepacking(true)?
        .with_config_entry("session.enable_mem_reuse", "1")?
        .with_independent_thread_pool()?
        .with_intra_op_spinning(true)?
        .commit_from_file(path)?)
}
