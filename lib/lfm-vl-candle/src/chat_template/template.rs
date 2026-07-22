//! Chat template rendering using minijinja.

use crate::error::{LfmError, Result};
use minijinja::{Environment, Value as MiniJinjaValue, context};

/// Fix incompatible Jinja2 syntax for minijinja.
fn fix_template(chat_template: &str) -> String {
    chat_template
        .replace(
            "content.startswith('')?>')",
            "content is startingwith('')?>')",
        )
        .replace(
            "content.endswith('')?>')",
            "content is endingwith('')?>')",
        )
        .replace(
            "content.split('?>')[0].rstrip('\\n').split('?')[-1].lstrip('\\n')",
            "((content | split('?>'))[0] | rstrip('\\n') | split('?'))[-1] | lstrip('\\n')",
        )
        .replace(
            "content.split('?>')[-1].lstrip('\\n')",
            "(content | split('?>'))[-1] | lstrip('\\n')",
        )
        .replace(
            "reasoning_content.strip('\\n')",
            "reasoning_content | strip('\\n')",
        )
        .replace(
            "content.lstrip('\\n')",
            "content | lstrip('\\n')",
        )
        .replace("{%- generation -%}", "")
        .replace("{%- endgeneration -%}", "")
}

/// Load the `bos_token` string from tokenizer_config.json.
fn load_bos_token(path: &str) -> Result<String> {
    let cfg_path = path.to_string() + "/tokenizer_config.json";
    let config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&cfg_path)?)?;
    config
        .get("bos_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| LfmError::Config("bos_token not found in tokenizer_config.json".into()))
}

/// Load the `eos_token` string from tokenizer_config.json.
fn load_eos_token(path: &str) -> Result<String> {
    let cfg_path = path.to_string() + "/tokenizer_config.json";
    let config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&cfg_path)?)?;
    config
        .get("eos_token")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| LfmError::Config("eos_token not found in tokenizer_config.json".into()))
}

fn load_template(path: &str) -> Result<String> {
    let tokenizer_config_file = path.to_string() + "/tokenizer_config.json";
    let chat_template = if std::path::Path::new(&tokenizer_config_file).exists() {
        let config: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&tokenizer_config_file)?)?;
        config["chat_template"].as_str().map(|s| s.to_string())
    } else {
        None
    };
    let chat_template = match chat_template {
        Some(t) => Some(t),
        None => {
            let jinja_path = path.to_string() + "/chat_template.jinja";
            if std::path::Path::new(&jinja_path).exists() {
                Some(std::fs::read_to_string(&jinja_path)?)
            } else {
                None
            }
        }
    };
    let chat_template = chat_template
        .ok_or_else(|| LfmError::Config("chat_template not found in model path".into()))?;
    Ok(fix_template(&chat_template))
}

/// Leak a String to &'static str for minijinja template registration.
fn string_to_static(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

/// Chat template renderer.
pub struct ChatTemplate {
    env: Environment<'static>,
    bos_token: String,
    eos_token: String,
}

impl ChatTemplate {
    /// Initialize from a model directory containing tokenizer_config.json
    /// or chat_template.jinja.
    pub fn init(path: &str) -> Result<Self> {
        let template = load_template(path)?;
        let template = string_to_static(template);
        let mut env = Environment::new();
        Self::setup_environment(&mut env);
        env.add_template("chat", template)?;
        let bos_token = load_bos_token(path)?;
        let eos_token = load_eos_token(path)?;
        Ok(Self { env, bos_token, eos_token })
    }

    fn setup_environment(env: &mut Environment<'static>) {
        env.add_filter("tojson", |v: MiniJinjaValue| {
            serde_json::to_string(&v).unwrap()
        });
        env.add_filter("split", |s: String, delimiter: String| {
            s.split(&delimiter)
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
        });
        env.add_filter("lstrip", |s: String, chars: Option<String>| match chars {
            Some(chars_str) => s.trim_start_matches(chars_str.as_str()).to_string(),
            None => s.trim_start().to_string(),
        });
        env.add_filter("rstrip", |s: String, chars: Option<String>| match chars {
            Some(chars_str) => s.trim_end_matches(chars_str.as_str()).to_string(),
            None => s.trim_end().to_string(),
        });
        env.add_filter("string", |v: MiniJinjaValue| -> String { format!("{}", v) });
    }

    /// Render the user content through the chat template.
    pub fn render(&self, user_content: &str) -> Result<String> {
        let messages = vec![serde_json::json!({
            "role": "user",
            "content": user_content
        })];
        let ctx = context! {
            messages => messages,
            add_generation_prompt => true,
            bos_token => self.bos_token,
            eos_token => self.eos_token,
        };
        let tmpl = self.env.get_template("chat")?;
        let text = tmpl
            .render(ctx)
            .map_err(|e| LfmError::Template(format!("render error: {e}")))?;
        Ok(text)
    }
}
