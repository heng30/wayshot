use std::time::Duration;
use video_editor::tracks::frame_position::*;

#[test]
fn test_fps_to_rational_ntsc() {
    assert_eq!(FramePosition::fps_to_rational(23.976), (24000, 1001));
    assert_eq!(FramePosition::fps_to_rational(29.97), (30000, 1001));
    assert_eq!(FramePosition::fps_to_rational(59.94), (60000, 1001));
}

#[test]
fn test_fps_to_rational_integer() {
    assert_eq!(FramePosition::fps_to_rational(24.0), (24, 1));
    assert_eq!(FramePosition::fps_to_rational(25.0), (25, 1));
    assert_eq!(FramePosition::fps_to_rational(30.0), (30, 1));
    assert_eq!(FramePosition::fps_to_rational(60.0), (60, 1));
}

#[test]
fn test_frame_position_duration_from_start() {
    // 24 FPS, 第 100 帧应该正好是 100/24 = 4.16666... 秒
    let pos = FramePosition::new(100, 24, 1);
    let duration = pos.duration_from_start();
    // 使用近似比较，因为 f64 精度有限
    assert!((duration.as_secs_f64() - 100.0 / 24.0).abs() < 0.0001);
}

#[test]
fn test_frame_position_frame_duration() {
    // 24 FPS, 每帧应该是 1/24 秒
    let pos = FramePosition::new(0, 24, 1);
    let duration = pos.frame_duration();
    // 使用近似比较，因为 f64 精度有限
    assert!((duration.as_secs_f64() - 1.0 / 24.0).abs() < 0.0001);
}

#[test]
fn test_time_to_frame_converter_round_trip() {
    // 测试往返转换：Duration → frame → Duration
    let converter = TimeToFrameConverter::from_f32(23.976);

    for frame_idx in [0, 100, 1000, 10000] {
        let duration = converter.frame_to_duration(frame_idx);
        let back_to_frame = converter.duration_to_frame(duration);

        assert_eq!(
            frame_idx, back_to_frame,
            "Round-trip failed for frame {}",
            frame_idx
        );
    }
}

#[test]
fn test_time_to_frame_converter_precision() {
    let converter = TimeToFrameConverter::from_f32(23.976);

    // 测试特定时间点的转换
    let duration = Duration::from_secs_f64(10.0);
    let frame = converter.duration_to_frame(duration);

    // 10 秒 @ 23.976 FPS 应该是 239.76 帧 → 四舍五入到 240 帧
    assert_eq!(frame, 240);

    // 往返转换验证（误差应小于 20ms，约半帧时间）
    let back_to_duration = converter.frame_to_duration(frame);
    assert!((back_to_duration.as_secs_f64() - 10.0).abs() < 0.02);
}

#[test]
fn test_frame_range_frame_count() {
    let start = FramePosition::new(100, 24, 1);
    let end = FramePosition::new(250, 24, 1);
    let range = FrameRange::new(start, end);

    assert_eq!(range.frame_count(), 150);
}

#[test]
fn test_frame_range_from_start_count() {
    let start = FramePosition::new(100, 30, 1);
    let range = FrameRange::from_start_count(start, 50);

    assert_eq!(range.start.frame_index, 100);
    assert_eq!(range.end.frame_index, 150);
    assert_eq!(range.frame_count(), 50);
}

#[test]
fn test_frame_position_add_sub_frames() {
    let pos = FramePosition::new(100, 24, 1);

    let pos2 = pos.add_frames(50);
    assert_eq!(pos2.frame_index, 150);

    let pos3 = pos2.sub_frames(50);
    assert_eq!(pos3.unwrap().frame_index, 100);

    // 测试下溢
    let pos4 = pos.sub_frames(150);
    assert!(pos4.is_none());
}
