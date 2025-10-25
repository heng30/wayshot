pub mod audio_processor;
pub mod mp4_processor;
pub mod sample_type;

pub use audio_processor::{
    AudioProcessor, AudioProcessorConfigBuilder, OutputDestination, sample_rate,
};
pub use mp4_processor::{AudioFrameType, Mp4Processor, Mp4ProcessorConfigBuilder, VideoFrameType};
pub use sample_type::{I24, SampleType};

pub use crossbeam::channel::{Receiver, Sender, bounded};
