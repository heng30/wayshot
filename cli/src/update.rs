use anyhow::{Result, anyhow};
use clap::Subcommand;
use serde_json::{Value, json};

const UPDATE_CHECK_URL: &str = "https://api.github.com/repos/heng30/wayshot/releases/latest";

#[derive(Subcommand)]
pub enum Cmd {
    /// 检查是否有新版本（读 GitHub releases，不下载安装）
    Check {
        /// 与哪个版本比较，缺省为当前 CLI 版本
        #[arg(long)]
        current: Option<String>,
    },
}

pub fn run(cmd: &Cmd) -> Result<Value> {
    match cmd {
        Cmd::Check { current } => check(current.as_deref()),
    }
}

fn check(current: Option<&str>) -> Result<Value> {
    let current = current.unwrap_or(env!("CARGO_PKG_VERSION")).to_string();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| anyhow!("创建 tokio runtime 失败: {e}"))?;

    let release = runtime.block_on(async {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| anyhow!("创建 HTTP 客户端失败: {e}"))?;

        let response = client
            .get(UPDATE_CHECK_URL)
            .headers(cutil::http::headers())
            .send()
            .await
            .map_err(|e| anyhow!("请求失败: {e}"))?
            .error_for_status()
            .map_err(|e| anyhow!("GitHub 返回错误: {e}"))?;

        response
            .json::<Value>()
            .await
            .map_err(|e| anyhow!("解析响应失败: {e}"))
    })?;

    let latest = release["tag_name"]
        .as_str()
        .ok_or_else(|| anyhow!("响应里没有 tag_name"))?
        .to_string();
    let has_update = version_greater(&latest, &current);

    Ok(json!({
        "current": current,
        "latest": latest,
        "has_update": has_update,
        "url": release["html_url"].as_str().unwrap_or_default(),
        "published_at": release["published_at"].as_str().unwrap_or_default(),
        "body": if has_update {
            release["body"].as_str().unwrap_or_default().to_string()
        } else {
            String::new()
        },
    }))
}

fn version_greater(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> {
        v.trim()
            .trim_start_matches(['v', 'V'])
            .split('.')
            .filter_map(|part| part.parse::<u64>().ok())
            .collect()
    };
    parse(latest) > parse(current)
}

#[cfg(test)]
mod tests {
    use super::version_greater;

    #[test]
    fn semantic_compare() {
        assert!(version_greater("v1.1.0", "v1.0.2"));
        assert!(version_greater("1.0.3", "v1.0.2"));
        assert!(version_greater("v1.10.0", "v1.9.0"));
        assert!(!version_greater("V1.0.2", "v1.0.2"));
        assert!(!version_greater("garbage", "v1.0.2"));
    }
}
