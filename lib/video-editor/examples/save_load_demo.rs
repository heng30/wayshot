//! 保存/加载项目演示示例
//!
//! 演示如何使用 video_editor 的序列化功能来保存和加载项目

use std::time::Duration;
use video_editor::tracks::manager::Manager;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

    println!("=== Video Editor 保存/加载演示 ===\n");

    // 创建一个空项目
    let mut manager = Manager::new();

    // 设置项目时长为 30 秒
    manager.duration = Duration::from_secs(30);

    println!("1. 创建了一个空项目（时长: 30秒）");
    println!("注意: serialization 模块尚未实现，此演示仅展示 Manager 的基本使用");

    println!("\n=== 演示完成 ===");

    Ok(())
}
