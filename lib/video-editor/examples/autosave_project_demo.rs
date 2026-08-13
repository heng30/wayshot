//! 自动保存项目演示示例
//!
//! 演示如何使用 video_editor 的自动保存功能

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use video_editor::{
    project::{
        AutoSaveConfig, AutoSaveManager, ManagerData, ProjectFile,
        check_for_recovery, check_recovery_on_startup, cleanup_recovery_file,
        get_all_recovery_files, restore_from_recovery,
    },
    tracks::manager::Manager,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

    println!("=== Video Editor 自动保存演示 ===\n");

    // 1. 创建自动保存配置
    println!("1. 创建自动保存配置");
    let config = AutoSaveConfig::new()
        .with_enabled(true)
        .with_interval(Duration::from_secs(5)) // 5秒间隔（演示用，实际应更长）
        .with_max_temp_files(3)
        .with_temp_location(PathBuf::from(".autosaves_demo"));

    println!("   - 自动保存已启用: {}", config.enabled);
    println!("   - 保存间隔: {:?}", config.interval);
    println!("   - 最大临时文件数: {}", config.max_temp_files);
    println!("   - 临时文件位置: {:?}", config.temp_location);

    // 2. 创建项目和自动保存管理器
    println!("\n2. 创建项目和自动保存管理器");
    let project_path = PathBuf::from("demo_project.json");
    let mut manager = Manager::new();
    manager.duration = Duration::from_secs(60);

    let manager_data = Arc::new(Mutex::new(ManagerData::new(manager)));

    let mut autosave_manager = AutoSaveManager::new(config, Some(&project_path))?;
    println!("   - 自动保存管理器已创建");

    // 3. 演示标记脏状态和手动保存
    println!("\n3. 标记项目有未保存更改");
    autosave_manager.mark_dirty();
    println!("   - 脏状态: {}", autosave_manager.is_dirty());

    // 4. 手动保存到临时文件
    println!("\n4. 手动保存到临时文件");
    let guard = manager_data.lock().unwrap();
    let project_file = ProjectFile::from(&*guard);
    drop(guard);

    let temp_path = autosave_manager.save_temp(&project_file)?;
    println!("   - 临时文件已保存到: {}", temp_path.display());
    println!("   - 脏状态: {}", autosave_manager.is_dirty());

    // 5. 再次修改并保存
    println!("\n5. 再次修改并保存");
    autosave_manager.mark_dirty();
    println!("   - 脏状态: {}", autosave_manager.is_dirty());

    let temp_path2 = autosave_manager.save_temp(&project_file)?;
    println!("   - 第二个临时文件: {}", temp_path2.display());

    // 6. 查看临时文件列表
    println!("\n6. 当前临时文件列表");
    for (i, path) in autosave_manager.get_temp_files()?.iter().enumerate() {
        println!("   [{}] {}", i + 1, path.display());
    }

    // 7. 检查是否应该自动保存
    println!("\n7. 检查自动保存条件");
    println!("   - 是否应该自动保存: {}", autosave_manager.should_autosave());

    // 标记脏状态后检查
    autosave_manager.mark_dirty();
    println!("   - 标记脏状态后: {}", autosave_manager.should_autosave());

    // 8. 启动后台自动保存线程
    println!("\n8. 启动后台自动保存线程");
    let manager_data_clone = Arc::clone(&manager_data);
    let handle = autosave_manager.start_autosave_thread(move || {
        let guard = manager_data_clone.lock().unwrap();
        Some(ProjectFile::from(&*guard))
    });
    println!("   - 自动保存线程已启动");
    println!("   - 线程运行状态: {}", handle.is_running());

    // 等待一段时间观察
    println!("\n   等待 3 秒...");
    std::thread::sleep(Duration::from_secs(3));

    // 9. 停止自动保存线程
    println!("\n9. 停止自动保存线程");
    handle.stop();
    println!("   - 线程已停止");

    // 10. 演示恢复功能
    println!("\n10. 演示恢复功能");
    let temp_dir = PathBuf::from(".autosaves_demo");

    // 检查特定项目的恢复文件
    if let Some(recovery) = check_for_recovery(&temp_dir, &project_path) {
        println!("   - 找到恢复文件: {}", recovery.temp_file_path.display());
        println!("   - 保存时间: {:?}", recovery.saved_at);
        println!("   - 文件大小: {} 字节", recovery.file_size);

        // 恢复项目
        match restore_from_recovery(&recovery) {
            Ok(_project) => println!("   - 项目恢复成功!"),
            Err(e) => println!("   - 恢复失败: {}", e),
        }

        // 清理恢复文件
        cleanup_recovery_file(&recovery)?;
        println!("   - 恢复文件已清理");
    } else {
        println!("   - 没有找到恢复文件");
    }

    // 11. 获取所有恢复文件
    println!("\n11. 获取所有恢复文件");
    let all_recovery = get_all_recovery_files(&temp_dir);
    println!("   - 找到 {} 个恢复文件", all_recovery.len());
    for (i, info) in all_recovery.iter().enumerate() {
        println!("   [{}] {} ({} 字节)", i + 1, info.temp_file_path.display(), info.file_size);
    }

    // 12. 检查启动时恢复
    println!("\n12. 检查启动时恢复");
    if let Some(recovery) = check_recovery_on_startup(&temp_dir, Some(&project_path)) {
        println!("   - 建议恢复: {}", recovery.temp_file_path.display());
    } else {
        println!("   - 没有需要恢复的项目");
    }

    // 13. 清理临时文件
    println!("\n13. 清理所有临时文件");
    autosave_manager.cleanup_temp_files()?;
    println!("   - 临时文件已清理");

    // 清理演示目录
    let _ = std::fs::remove_dir_all(".autosaves_demo");

    println!("\n=== 演示完成 ===");
    Ok(())
}