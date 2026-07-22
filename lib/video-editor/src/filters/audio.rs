pub mod compressor;
pub mod copy_channel;
pub mod denoise;
pub mod fade_in;
pub mod fade_out;
pub mod gain;
pub mod limiter;
pub mod mute;
pub mod noise_gate;
pub mod normalize;
pub mod speed;
pub mod voice_changer;

pub use compressor::CompressorFilter;
pub use copy_channel::{CopyChannelFilter, CopyDirection};
pub use denoise::DenoiseFilter;
pub use fade_in::FadeInFilter;
pub use fade_out::FadeOutFilter;
pub use gain::GainFilter;
pub use limiter::LimiterFilter;
pub use mute::{MuteChannel, MuteFilter};
pub use noise_gate::NoiseGateFilter;
pub use normalize::NormalizeFilter;
pub use speed::SpeedFilter as AudioSpeedFilter;
pub use voice_changer::VoiceChangerFilter;

pub fn all_filter_names() -> &'static [&'static str] {
    &[
        GainFilter::NAME,
        DenoiseFilter::NAME,
        FadeInFilter::NAME,
        FadeOutFilter::NAME,
        NormalizeFilter::NAME,
        MuteFilter::NAME,
        CopyChannelFilter::NAME,
        LimiterFilter::NAME,
        NoiseGateFilter::NAME,
        CompressorFilter::NAME,
        VoiceChangerFilter::NAME,
        AudioSpeedFilter::NAME,
    ]
}
