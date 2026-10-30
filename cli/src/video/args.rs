use clap::ValueEnum;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum SceneAlgorithm {
    Content,
    Adaptive,
    Histogram,
    Threshold,
}
