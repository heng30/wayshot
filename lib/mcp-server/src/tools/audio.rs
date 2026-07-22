use crate::{VideoEditorServer, service};
use rmcp::{
    ErrorData,
    handler::server::router::tool::{AsyncTool, ToolBase},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Deserialize, Default, JsonSchema)]
pub struct AudioRecordStartParams {
    pub save_dir: Option<String>,
}

#[derive(Serialize, JsonSchema)]
pub struct AudioRecordStartOutput {
    pub result: serde_json::Value,
}

pub struct AudioRecordStartTool;

impl ToolBase for AudioRecordStartTool {
    type Parameter = AudioRecordStartParams;
    type Output = AudioRecordStartOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_audio_record_start".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Start audio recording to a directory".into())
    }
}

impl AsyncTool<VideoEditorServer> for AudioRecordStartTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::audio::record_start(params.save_dir).map_err(ErrorData::from)?;
        Ok(AudioRecordStartOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct AudioRecordStopParams {}

#[derive(Serialize, JsonSchema)]
pub struct AudioRecordStopOutput {
    pub result: serde_json::Value,
}

pub struct AudioRecordStopTool;

impl ToolBase for AudioRecordStopTool {
    type Parameter = AudioRecordStopParams;
    type Output = AudioRecordStopOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_audio_record_stop".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Stop audio recording".into())
    }
}

impl AsyncTool<VideoEditorServer> for AudioRecordStopTool {
    async fn invoke(
        _server: &VideoEditorServer,
        _params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::audio::record_stop().map_err(ErrorData::from)?;
        Ok(AudioRecordStopOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct AudioStemSplitParams {
    pub audio_path: String,
}

#[derive(Serialize, JsonSchema)]
pub struct AudioStemSplitOutput {
    pub result: serde_json::Value,
}

pub struct AudioStemSplitTool;

impl ToolBase for AudioStemSplitTool {
    type Parameter = AudioStemSplitParams;
    type Output = AudioStemSplitOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_audio_stem_split".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Split audio into stems (vocals, drums, etc.) using AI (starts async task)".into())
    }
}

impl AsyncTool<VideoEditorServer> for AudioStemSplitTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::audio::stem_split(params.audio_path).map_err(ErrorData::from)?;
        Ok(AudioStemSplitOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct AudioTtsGenerateParams {
    pub text: String,
    pub index: Option<usize>,
}

#[derive(Serialize, JsonSchema)]
pub struct AudioTtsGenerateOutput {
    pub result: serde_json::Value,
}

pub struct AudioTtsGenerateTool;

impl ToolBase for AudioTtsGenerateTool {
    type Parameter = AudioTtsGenerateParams;
    type Output = AudioTtsGenerateOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_audio_tts_generate".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Generate speech from text using TTS (starts async task)".into())
    }
}

impl AsyncTool<VideoEditorServer> for AudioTtsGenerateTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result =
            service::audio::tts_generate(params.text, params.index).map_err(ErrorData::from)?;
        Ok(AudioTtsGenerateOutput { result })
    }
}

#[derive(Deserialize, JsonSchema, Default)]
pub struct AudioVadDetectParams {
    pub audio_path: String,
}

#[derive(Serialize, JsonSchema)]
pub struct AudioVadDetectOutput {
    pub result: serde_json::Value,
}

pub struct AudioVadDetectTool;

impl ToolBase for AudioVadDetectTool {
    type Parameter = AudioVadDetectParams;
    type Output = AudioVadDetectOutput;
    type Error = ErrorData;
    fn name() -> Cow<'static, str> {
        "ve_audio_vad_detect".into()
    }
    fn description() -> Option<Cow<'static, str>> {
        Some("Detect voice activity segments in an audio file".into())
    }
}

impl AsyncTool<VideoEditorServer> for AudioVadDetectTool {
    async fn invoke(
        _server: &VideoEditorServer,
        params: Self::Parameter,
    ) -> Result<Self::Output, Self::Error> {
        let result = service::audio::vad_detect(params.audio_path).map_err(ErrorData::from)?;
        Ok(AudioVadDetectOutput { result })
    }
}
