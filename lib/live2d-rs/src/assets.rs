use std::{fmt, fs, path::Path};

use crate::{
    json::{Model3, Pose3},
    moc3::{
        Moc3ArtMeshKeyforms, Moc3ArtMeshes, Moc3CanvasInfo, Moc3Deformers, Moc3DrawOrderGroups,
        Moc3DrawableMesh, Moc3Glues, Moc3Ids, Moc3KeyformBindings, Moc3OffscreenInfo, Moc3Parts,
        build_moc3_drawable_meshes_for_default_pose_with_offscreen_state,
    },
    runtime::ModelRuntime,
};

#[derive(Debug, Clone)]
pub struct DefaultModel {
    model: Model3,
    canvas: Moc3CanvasInfo,
    meshes: Vec<Moc3DrawableMesh>,
    textures: Vec<DecodedTexture>,
}

impl DefaultModel {
    pub fn model(&self) -> &Model3 {
        &self.model
    }

    pub fn canvas(&self) -> Moc3CanvasInfo {
        self.canvas
    }

    pub fn meshes(&self) -> &[Moc3DrawableMesh] {
        &self.meshes
    }

    pub fn textures(&self) -> &[DecodedTexture] {
        &self.textures
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedTexture {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl DecodedTexture {
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        Self {
            width,
            height,
            rgba,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

#[derive(Debug)]
pub enum AssetLoadError {
    Io {
        path: String,
        source: std::io::Error,
    },
    Json(crate::Error),
    Moc3(crate::Error),
    Image {
        path: String,
        source: image::ImageError,
    },
    MissingParent {
        path: String,
    },
    DrawableMeshes,
}

impl fmt::Display for AssetLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(formatter, "failed to read {path}: {source}"),
            Self::Json(error) => write!(formatter, "failed to parse model json: {error}"),
            Self::Moc3(error) => write!(formatter, "failed to parse moc3: {error}"),
            Self::Image { path, source } => write!(formatter, "failed to decode {path}: {source}"),
            Self::MissingParent { path } => write!(formatter, "path has no parent: {path}"),
            Self::DrawableMeshes => formatter.write_str("failed to build drawable meshes"),
        }
    }
}

impl std::error::Error for AssetLoadError {}

pub fn load_model(path: impl AsRef<Path>) -> Result<DefaultModel, AssetLoadError> {
    let parsed = parse_model(path)?;
    let mut meshes = build_moc3_drawable_meshes_for_default_pose_with_offscreen_state(
        &parsed.art_meshes,
        &parsed.art_mesh_keyforms,
        &parsed.deformers,
        &parsed.bindings,
        &parsed.ids,
        &parsed.offscreen,
    )
    .ok_or(AssetLoadError::DrawableMeshes)?;
    parsed
        .glues
        .apply(
            &mut meshes,
            &parsed.bindings,
            parsed.bindings.parameter_default_values(),
        )
        .ok_or(AssetLoadError::DrawableMeshes)?;

    Ok(DefaultModel {
        model: parsed.model,
        canvas: parsed.canvas,
        meshes,
        textures: parsed.textures,
    })
}

pub fn load_model_runtime(path: impl AsRef<Path>) -> Result<RuntimeModel, AssetLoadError> {
    let path = path.as_ref();
    let model_dir = path.parent().map(Path::to_path_buf);
    let parsed = parse_model(path)?;
    let runtime = ModelRuntime::new(
        parsed.model,
        parsed.canvas,
        parsed.art_meshes,
        parsed.art_mesh_keyforms,
        parsed.deformers,
        parsed.bindings,
        parsed.ids,
        parsed.offscreen,
        parsed.glues,
        parsed.parts,
        parsed.draw_order_groups,
        parsed.pose,
    )
    .ok_or(AssetLoadError::DrawableMeshes)?;

    Ok(RuntimeModel {
        runtime,
        textures: parsed.textures,
        model_dir,
    })
}

#[derive(Debug, Clone)]
pub struct RuntimeModel {
    runtime: ModelRuntime,
    textures: Vec<DecodedTexture>,
    model_dir: Option<std::path::PathBuf>,
}

impl RuntimeModel {
    pub fn runtime(&self) -> &ModelRuntime {
        &self.runtime
    }

    pub fn runtime_mut(&mut self) -> &mut ModelRuntime {
        &mut self.runtime
    }

    pub fn textures(&self) -> &[DecodedTexture] {
        &self.textures
    }

    pub fn model_dir(&self) -> Option<&Path> {
        self.model_dir.as_deref()
    }
}

struct ParsedModel {
    model: Model3,
    canvas: Moc3CanvasInfo,
    art_meshes: Moc3ArtMeshes,
    art_mesh_keyforms: Moc3ArtMeshKeyforms,
    deformers: Moc3Deformers,
    bindings: Moc3KeyformBindings,
    ids: Moc3Ids,
    offscreen: Moc3OffscreenInfo,
    glues: Moc3Glues,
    parts: Moc3Parts,
    draw_order_groups: Option<Moc3DrawOrderGroups>,
    pose: Option<Pose3>,
    textures: Vec<DecodedTexture>,
}

fn parse_model(path: impl AsRef<Path>) -> Result<ParsedModel, AssetLoadError> {
    let path = path.as_ref();
    let model_source = read_text(path)?;
    let model = Model3::from_json_str(&model_source).map_err(AssetLoadError::Json)?;
    let model_dir = path.parent().ok_or_else(|| AssetLoadError::MissingParent {
        path: path.display().to_string(),
    })?;
    let moc_path = model_dir.join(model.moc());
    let moc = read_bytes(&moc_path)?;

    let art_meshes = Moc3ArtMeshes::parse(&moc).map_err(AssetLoadError::Moc3)?;
    let art_mesh_keyforms = Moc3ArtMeshKeyforms::parse(&moc).map_err(AssetLoadError::Moc3)?;
    let deformers = Moc3Deformers::parse(&moc).map_err(AssetLoadError::Moc3)?;
    let bindings = Moc3KeyformBindings::parse(&moc).map_err(AssetLoadError::Moc3)?;
    let ids = Moc3Ids::parse(&moc).map_err(AssetLoadError::Moc3)?;
    let offscreen = Moc3OffscreenInfo::parse(&moc).map_err(AssetLoadError::Moc3)?;
    let glues = Moc3Glues::parse(&moc).map_err(AssetLoadError::Moc3)?;
    let parts = Moc3Parts::parse(&moc).map_err(AssetLoadError::Moc3)?;
    let canvas = Moc3CanvasInfo::parse(&moc).map_err(AssetLoadError::Moc3)?;
    let draw_order_groups = Moc3DrawOrderGroups::parse(&moc).map_err(AssetLoadError::Moc3)?;
    let pose = match model.pose() {
        Some(pose_file) => {
            let pose_source = read_text(&model_dir.join(pose_file))?;
            Some(Pose3::from_json_str(&pose_source).map_err(AssetLoadError::Json)?)
        }
        None => None,
    };
    let textures = model
        .textures()
        .iter()
        .map(|texture| decode_texture(model_dir.join(texture)))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ParsedModel {
        model,
        canvas,
        art_meshes,
        art_mesh_keyforms,
        deformers,
        bindings,
        ids,
        offscreen,
        glues,
        parts,
        draw_order_groups,
        pose,
        textures,
    })
}

fn read_text(path: &Path) -> Result<String, AssetLoadError> {
    fs::read_to_string(path).map_err(|source| AssetLoadError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, AssetLoadError> {
    fs::read(path).map_err(|source| AssetLoadError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn decode_texture(path: impl AsRef<Path>) -> Result<DecodedTexture, AssetLoadError> {
    let path = path.as_ref();
    let image = image::open(path).map_err(|source| AssetLoadError::Image {
        path: path.display().to_string(),
        source,
    })?;
    let rgba = image.to_rgba8();
    Ok(DecodedTexture::new(
        rgba.width(),
        rgba.height(),
        rgba.into_raw(),
    ))
}
