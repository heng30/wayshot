# GPT-SoVITS 模型架构说明

## 模型配置

`GptSoVitsModelConfig` 包含以下模型路径配置：

```rust
pub struct GptSoVitsModelConfig {
    pub sovits_path: PathBuf,         // custom_vits.onnx
    pub ssl_path: PathBuf,            // ssl.onnx
    pub t2s_encoder_path: PathBuf,    // custom_t2s_encoder.onnx
    pub t2s_fs_decoder_path: PathBuf, // custom_t2s_fs_decoder.onnx
    pub t2s_s_decoder_path: PathBuf,  // custom_t2s_s_decoder.onnx
    pub bert_path: Option<PathBuf>,   // bert.onnx
    pub g2pw_path: Option<PathBuf>,   // g2pW.onnx
    pub g2p_en_path: Option<PathBuf>, // g2p_en
}
```

## 核心模型（必需）

### 1. sovits_path - `custom_vits.onnx`
**用途**：VITS 声器模型，最终音频生成器

- **输入**：
  - 文本序列（phone_ids）
  - 预测的语义特征（pred_semantic）
  - 参考音频（ref_audio_32k）

- **输出**：生成的音频波形（32kHz采样率）

- **功能**：
  - 将语义 tokens 转换为实际音频波形
  - 保留参考音频的音色和韵律特征
  - 生成高质量语音

### 2. ssl_path - `ssl.onnx`
**用途**：SSL（Self-Supervised Learning）特征提取器

- **输入**：参考音频（16kHz采样率）

- **输出**：SSL 内容特征（多维向量）

- **功能**：
  - 从参考音频中提取说话人特征
  - 捕捉音色、语调等个性化信息
  - 为后续模型提供说话人嵌入

### 3. t2s_encoder_path - `custom_t2s_encoder.onnx`
**用途**：Text2Semantic 编码器

- **输入**：参考音频的 SSL 特征

- **输出**：提示序列（prompts）

- **功能**：
  - 将参考音频编码为语义提示
  - 建立说话人风格的基础表示
  - 引导后续的语义生成

### 4. t2s_fs_decoder_path - `custom_t2s_fs_decoder.onnx`
**用途**：Text2Semantic 第一步解码器（First Stage Decoder）

- **输入**：
  - 文本序列（x）
  - 提示序列（prompts）
  - BERT 特征（bert）

- **输出**：
  - 初始 logits
  - KV cache（用于后续解码）

- **功能**：
  - 处理初始输入
  - 建立键值（KV）缓存
  - 为自回归解码做准备

### 5. t2s_s_decoder_path - `custom_t2s_s_decoder.onnx`
**用途**：Text2Semantic 第二步自回归解码器（Second Stage Decoder）

- **输入**：
  - 历史生成的 tokens（iy）
  - KV cache
  - 当前索引（idx）
  - 前缀长度（y_len）

- **输出**：下一个语义 token 的 logits

- **功能**：
  - 逐个自回归生成语义 tokens
  - 使用 KV cache 优化计算
  - 支持 EOS（End of Sequence）检测

---

## 辅助模型（可选但推荐）

### 6. bert_path - `bert.onnx`（可选）
**用途**：BERT 语言模型，用于文本特征提取

- **输入**：
  - 文本字符串
  - word2ph（词到音素的映射）

- **输出**：1024 维特征向量

- **功能**：
  - 提供文本的语义特征
  - 改善韵律和语调
  - 提升语音自然度

- **使用位置**：
  - **参考音频文字**：提取参考音频对应文本的语义特征（`ref_bert`）
  - **待合成文本**：提取待合成文本的语义特征（`bert`）

**注意**：如果 BERT 模型未提供或失败，系统会使用零向量代替。

### 7. g2pw_path - `g2pW.onnx`（可选）
**用途**：中文 Grapheme-to-Phoneme-to-Word 模型

- **输入**：中文字符串

- **输出**：拼音序列

- **功能**：
  - 将中文字符转换为拼音
  - 处理多音字
  - 提供准确的音素信息

- **使用位置**：
  - **参考音频文字**：将参考音频对应的中文文本转换为拼音，生成 `ref_seq`
  - **待合成文本**：将待合成的中文文本转换为拼音，生成 `text_seq`

**替代方案**：如果不提供，会使用规则引擎进行简单转换。

### 8. g2p_en_path - `g2p_en`（可选）
**用途**：英文 Grapheme-to-Phoneme 模型

- **输入**：英文文本

- **输出**：音素序列

- **功能**：
  - 处理英文单词的发音
  - 支持混合语言文本

- **使用位置**：
  - **参考音频文字**：将参考音频对应的英文文本转换为音素
  - **待合成文本**：将待合成的英文文本转换为音素

**注意**：目前仍处于实验阶段，效果可能不如中文。

---

## 工作流程

```
待合成文本："你好，世界"
                 ↓
┌─────────────────────────────────────────────────────┐
│ 文本处理（G2PW/G2pEn + BERT）                       │
│ • G2PW/G2pEn → 转换为音素序列（text_seq）           │
│ • BERT → 提取语义特征（bert）                       │
└─────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────┐
│ 参考音频（如：me.wav）                              │
└────────────────┬────────────────────────────────────┘
                 ↓
         ┌────────────────┐
         │  SSL (ssl.onnx)│ → 提取说话人特征
         └────────────────┘

┌─────────────────────────────────────────────────────┐
│ 参考音频文字："这是一段参考音频"                    │
└────────────────┬────────────────────────────────────┘
                 ↓
┌─────────────────────────────────────────────────────┐
│ 文本处理（G2PW/G2pEn + BERT）                       │
│ • G2PW/G2pEn → 转换为音素序列（ref_seq）            │
│ • BERT → 提取语义特征（ref_bert）                   │
└─────────────────────────────────────────────────────┘

       ┌──────────────────┐
       │ T2S Encoder      │ → 编码参考音频（使用 SSL 特征）
       └──────────┬───────┘
                  ↓
       ┌────────────────────────────────────────────────┐
       │ T2S FS Decoder                                 │ → 初始解码 + 建立 KV cache
       │ (使用：ref_seq + text_seq + ref_bert + bert)   │
       └──────────┬─────────────────────────────────────┘
                  ↓
       ┌──────────────────────┐
       │ T2S S Decoder        │ → 自回归生成语义 tokens
       │ (循环调用)           │
       └──────────┬───────────┘
                  ↓
       ┌──────────────────────────────────────────────────┐
       │  VITS (sovits)                                   │ → 生成最终音频
       │ (使用：tokens + text_seq + ref_audio_32k)        │
       └──────────┬───────────────────────────────────────┘
                  ↓
       ┌────────────────┐
       │  音频输出      │
       └────────────────┘
```

### 关键点说明：

1. **G2PW/G2pEn 和 BERT 被使用两次**：
   - 第一次：处理参考音频对应的文字，生成 `ref_seq` 和 `ref_bert`
   - 第二次：处理待合成的文本，生成 `text_seq` 和 `bert`

2. **数据流向**：
   - 参考音频 → SSL → 提取说话人特征
   - 参考音频文字 → G2PW/G2pEn + BERT → `ref_seq` + `ref_bert`
   - 待合成文本 → G2PW/G2pEn + BERT → `text_seq` + `bert`
   - 所有这些数据都输入到 T2S 模型中生成语义 tokens
   - 最后 VITS 使用语义 tokens 生成音频

---

## 模型依赖关系

```
依赖链：

t2s_s_decoder
    ↓ 依赖
t2s_fs_decoder
    ↓ 依赖
t2s_encoder
    ↓ 依赖
   ssl
    ↓ 需要
参考音频
    ↓
sovits ← 使用所有上述模型的输出 + BERT 特征
```

### 依赖详解

1. **t2s_s_decoder** 需要 **t2s_fs_decoder** 的输出
   - FS decoder 提供初始 KV cache
   - S decoder 继续自回归生成

2. **t2s_fs_decoder** 需要 **t2s_encoder** 的输出
   - Encoder 提供提示序列
   - FS decoder 基于提示开始解码
   - 同时需要：参考音频文字的 `ref_seq`、待合成文本的 `text_seq`、两者的 BERT 特征

3. **t2s_encoder** 需要 **ssl** 的输出
   - SSL 从参考音频提取特征
   - Encoder 将特征编码为提示

4. **文本处理**（G2PW/G2pEn + BERT）被调用两次
   - **参考音频文字**：生成 `ref_seq`（音素序列）和 `ref_bert`（语义特征）
   - **待合成文本**：生成 `text_seq`（音素序列）和 `bert`（语义特征）
   - 这两组数据在 T2S FS Decoder 中拼接使用

5. **sovits** 需要所有模型的输出
   - T2S 生成的语义 tokens（`pred_semantic`）
   - 待合成文本的音素序列（`text_seq`）
   - 参考音频（`ref_audio_32k`）

---

## 最小配置

**最少需要 5 个核心模型**：
1. `sovits_path` - VITS 音频生成
2. `ssl_path` - 说话人特征提取
3. `t2s_encoder_path` - 语义编码
4. `t2s_fs_decoder_path` - 初始解码
5. `t2s_s_decoder_path` - 自回归解码

**可选 3 个辅助模型**（推荐全部使用以获得最佳效果）：
6. `bert_path` - 文本语义特征
7. `g2pw_path` - 中文拼音转换
8. `g2p_en_path` - 英文音素转换

---

## 模型下载地址

官方模型下载地址：
1. https://huggingface.co/mikv39/gpt-sovits-onnx-custom
2. https://huggingface.co/cisco-ai/mini-bart-g2p/tree/main/onnx

---

## 技术细节

### T2S Decoder 工作原理

T2S（Text2Semantic）模型负责将文本转换为语义 tokens：

1. **FS Decoder（First Stage）**：
   - 处理完整的输入序列
   - 生成第一个语义 token
   - 初始化 KV cache

2. **S Decoder（Second Stage）**：
   - 自回归生成后续 tokens
   - 动态扩展 KV cache
   - 直到遇到 EOS token 或达到最大步数

### KV Cache 优化

- **初始大小**：由 FS decoder 确定
- **增量扩展**：每次增加 1024 个位置（`CACHE_REALLOC_INCREMENT`）
- **优势**：避免重复计算，提升推理速度

### 音频参数

- **采样率**：32kHz（VITS 输出）
- **参考音频采样率**：支持任意采样率，自动重采样到 16kHz 和 32kHz
- **参考音频最小长度**：8000 samples（0.5秒 @ 16kHz）
