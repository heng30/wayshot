const DFN2_FOLDER: &str = "dfn2";
const DFN2_LL_FOLDER: &str = "dfn2_ll";
const DFN2_H0_FOLDER: &str = "dfn2_h0";
const DFN3_FOLDER: &str = "dfn3";
const DFN3_LL_FOLDER: &str = "dfn3_ll";
const DFN3_H0_FOLDER: &str = "dfn3_h0";

// Model source: https://github.com/heng30/deepfilter-rt/tree/master/models
const DFN2_URL: &str = "https://github.com/heng30/deepfilter-rt/tree/master/models/dfn2";
const DFN2_LL_URL: &str = "https://github.com/heng30/deepfilter-rt/tree/master/models/dfn2_ll";
const DFN2_H0_URL: &str = "https://github.com/heng30/deepfilter-rt/tree/master/models/dfn2_h0";
const DFN3_URL: &str = "https://github.com/heng30/deepfilter-rt/tree/master/models/dfn3";
const DFN3_LL_URL: &str = "https://github.com/heng30/deepfilter-rt/tree/master/models/dfn3_ll";
const DFN3_H0_URL: &str = "https://github.com/heng30/deepfilter-rt/tree/master/models/dfn3_h0";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Model {
    /// Standard DeepFilterNet2 (30ms latency, stateless)
    DeepFilterNet2,
    /// Low-latency DeepFilterNet2 (10ms latency, stateless)
    DeepFilterNet2LL,
    /// Stateful DeepFilterNet2 with GRU (30ms latency, best quality)
    DeepFilterNet2H0,
    /// Standard DeepFilterNet3 (30ms latency, stateless)
    DeepFilterNet3,
    /// Low-latency DeepFilterNet3 (10ms latency, stateless)
    DeepFilterNet3LL,
    /// Stateful DeepFilterNet3 with GRU (30ms latency, best quality)
    DeepFilterNet3H0,
}

impl Model {
    pub fn all_models() -> Vec<Self> {
        vec![
            Self::DeepFilterNet2,
            Self::DeepFilterNet2LL,
            Self::DeepFilterNet2H0,
            Self::DeepFilterNet3,
            Self::DeepFilterNet3LL,
            Self::DeepFilterNet3H0,
        ]
    }

    /// Human-readable name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::DeepFilterNet2 => "DeepFilterNet2",
            Self::DeepFilterNet2LL => "DeepFilterNet2-LL",
            Self::DeepFilterNet2H0 => "DeepFilterNet2-H0",
            Self::DeepFilterNet3 => "DeepFilterNet3",
            Self::DeepFilterNet3LL => "DeepFilterNet3-LL",
            Self::DeepFilterNet3H0 => "DeepFilterNet3-H0",
        }
    }

    /// Model folder name (used as directory name under models/).
    pub fn folder_name(&self) -> &'static str {
        match self {
            Self::DeepFilterNet2 => DFN2_FOLDER,
            Self::DeepFilterNet2LL => DFN2_LL_FOLDER,
            Self::DeepFilterNet2H0 => DFN2_H0_FOLDER,
            Self::DeepFilterNet3 => DFN3_FOLDER,
            Self::DeepFilterNet3LL => DFN3_LL_FOLDER,
            Self::DeepFilterNet3H0 => DFN3_H0_FOLDER,
        }
    }

    /// Try to detect model from folder name.
    pub fn try_from_folder(folder: &str) -> Option<Self> {
        match folder.to_ascii_lowercase().as_str() {
            DFN2_FOLDER => Some(Self::DeepFilterNet2),
            DFN2_LL_FOLDER => Some(Self::DeepFilterNet2LL),
            DFN2_H0_FOLDER => Some(Self::DeepFilterNet2H0),
            DFN3_FOLDER => Some(Self::DeepFilterNet3),
            DFN3_LL_FOLDER => Some(Self::DeepFilterNet3LL),
            DFN3_H0_FOLDER => Some(Self::DeepFilterNet3H0),
            _ => None,
        }
    }

    /// Download URL for the model files.
    ///
    /// Each variant has its own folder in the model repository.
    /// The URL points to the GitHub tree view; individual .onnx files
    /// can be fetched via raw.githubusercontent.com.
    pub fn download_url(&self) -> &'static str {
        match self {
            Self::DeepFilterNet2 => DFN2_URL,
            Self::DeepFilterNet2LL => DFN2_LL_URL,
            Self::DeepFilterNet2H0 => DFN2_H0_URL,
            Self::DeepFilterNet3 => DFN3_URL,
            Self::DeepFilterNet3LL => DFN3_LL_URL,
            Self::DeepFilterNet3H0 => DFN3_H0_URL,
        }
    }

    /// Try to detect model from download URL.
    pub fn try_from_url(url: &str) -> Option<Self> {
        match url {
            DFN2_URL => Some(Self::DeepFilterNet2),
            DFN2_LL_URL => Some(Self::DeepFilterNet2LL),
            DFN2_H0_URL => Some(Self::DeepFilterNet2H0),
            DFN3_URL => Some(Self::DeepFilterNet3),
            DFN3_LL_URL => Some(Self::DeepFilterNet3LL),
            DFN3_H0_URL => Some(Self::DeepFilterNet3H0),
            _ => None,
        }
    }

    /// Whether this is a low-latency variant (10ms vs 30ms).
    pub fn is_low_latency(&self) -> bool {
        matches!(self, Self::DeepFilterNet2LL | Self::DeepFilterNet3LL)
    }

    /// Whether this is a stateful (H0/GRU) variant.
    pub fn is_stateful(&self) -> bool {
        matches!(self, Self::DeepFilterNet2H0 | Self::DeepFilterNet3H0)
    }

    /// Algorithmic latency in milliseconds.
    pub fn latency_ms(&self) -> u32 {
        if self.is_low_latency() { 10 } else { 30 }
    }

    /// Convert to the existing `ModelVariant` used by `DeepFilterProcessor`.
    pub fn to_variant(&self) -> crate::ModelVariant {
        match self {
            Self::DeepFilterNet2 => crate::ModelVariant::DeepFilterNet2,
            Self::DeepFilterNet2LL => crate::ModelVariant::DeepFilterNet2LL,
            Self::DeepFilterNet2H0 => crate::ModelVariant::DeepFilterNet2H0,
            Self::DeepFilterNet3 => crate::ModelVariant::DeepFilterNet3,
            Self::DeepFilterNet3LL => crate::ModelVariant::DeepFilterNet3LL,
            Self::DeepFilterNet3H0 => crate::ModelVariant::DeepFilterNet3H0,
        }
    }

    /// Convert from `ModelVariant` used by `DeepFilterProcessor`.
    pub fn from_variant(variant: crate::ModelVariant) -> Self {
        match variant {
            crate::ModelVariant::DeepFilterNet2 => Self::DeepFilterNet2,
            crate::ModelVariant::DeepFilterNet2LL => Self::DeepFilterNet2LL,
            crate::ModelVariant::DeepFilterNet2H0 => Self::DeepFilterNet2H0,
            crate::ModelVariant::DeepFilterNet3 => Self::DeepFilterNet3,
            crate::ModelVariant::DeepFilterNet3LL => Self::DeepFilterNet3LL,
            crate::ModelVariant::DeepFilterNet3H0 => Self::DeepFilterNet3H0,
        }
    }
}

impl std::fmt::Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}
