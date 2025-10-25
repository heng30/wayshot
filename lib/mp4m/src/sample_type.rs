use hound::SampleFormat;

/// Trait for sample types that can be converted to and from f32
pub trait SampleType: Copy {
    fn to_f32(self) -> f32;
    fn from_f32(value: f32) -> Self;
    fn max() -> Self;
    fn bits_per_sample() -> u16;
    fn sample_format() -> SampleFormat;
}

// Implement SampleType for common audio sample types
impl SampleType for f32 {
    #[inline]
    fn to_f32(self) -> f32 {
        self
    }

    #[inline]
    fn from_f32(value: f32) -> Self {
        value
    }

    #[inline]
    fn max() -> Self {
        f32::MAX
    }

    #[inline]
    fn bits_per_sample() -> u16 {
        32
    }

    #[inline]
    fn sample_format() -> SampleFormat {
        SampleFormat::Float
    }
}

impl SampleType for i16 {
    #[inline]
    fn to_f32(self) -> f32 {
        self as f32
    }

    #[inline]
    fn from_f32(value: f32) -> Self {
        value as i16
    }

    #[inline]
    fn max() -> Self {
        i16::MAX
    }

    #[inline]
    fn bits_per_sample() -> u16 {
        16
    }

    #[inline]
    fn sample_format() -> SampleFormat {
        SampleFormat::Int
    }
}

impl SampleType for i32 {
    #[inline]
    fn to_f32(self) -> f32 {
        self as f32
    }

    #[inline]
    fn from_f32(value: f32) -> Self {
        value as i32
    }

    #[inline]
    fn max() -> Self {
        i32::MAX
    }

    #[inline]
    fn bits_per_sample() -> u16 {
        32
    }

    #[inline]
    fn sample_format() -> SampleFormat {
        SampleFormat::Int
    }
}

#[derive(Clone, Copy, Debug)]
pub struct I24(i32);

impl I24 {
    const MIN: i32 = -(1 << 23);
    const MAX: i32 = (1 << 23) - 1;
}

impl SampleType for I24 {
    #[inline]
    fn to_f32(self) -> f32 {
        self.0 as f32
    }

    #[inline]
    fn from_f32(value: f32) -> Self {
        I24((value as i32).clamp(Self::MIN, Self::max().0))
    }

    #[inline]
    fn max() -> Self {
        I24(Self::MAX)
    }

    #[inline]
    fn bits_per_sample() -> u16 {
        24
    }

    #[inline]
    fn sample_format() -> SampleFormat {
        SampleFormat::Int
    }
}
