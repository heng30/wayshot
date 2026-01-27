// cargo test -p video-utils --test subtitle_test

use video_utils::subtitle::chinese_numbers_to_primitive_numbers;

#[test]
fn test_chinese_numbers_simple() {
    // 测试简单中文数字
    assert_eq!(chinese_numbers_to_primitive_numbers("五"), "5");
    assert_eq!(chinese_numbers_to_primitive_numbers("十"), "10");
    assert_eq!(chinese_numbers_to_primitive_numbers("一百"), "100");
    assert_eq!(chinese_numbers_to_primitive_numbers("一千"), "1000");
    assert_eq!(chinese_numbers_to_primitive_numbers("一万"), "10000");
}

#[test]
fn test_chinese_numbers_complex() {
    // 测试复杂中文数字
    assert_eq!(chinese_numbers_to_primitive_numbers("十五"), "15");
    assert_eq!(chinese_numbers_to_primitive_numbers("三十五"), "35");
    assert_eq!(chinese_numbers_to_primitive_numbers("一百二十三"), "123");
    assert_eq!(
        chinese_numbers_to_primitive_numbers("一千二百三十四"),
        "1234"
    );
    assert_eq!(
        chinese_numbers_to_primitive_numbers("一万二千三百四十五"),
        "12345"
    );
}

#[test]
fn test_mixed_text() {
    // 测试混合文本
    assert_eq!(
        chinese_numbers_to_primitive_numbers("我有五本书"),
        "我有5本书"
    );
    assert_eq!(
        chinese_numbers_to_primitive_numbers("今天十五号"),
        "今天15号"
    );
    assert_eq!(
        chinese_numbers_to_primitive_numbers("价格是一千二百元"),
        "价格是1200元"
    );
    assert_eq!(
        chinese_numbers_to_primitive_numbers("第三章：基础教程"),
        "第3章：基础教程"
    );
}

#[test]
fn test_yi_context() {
    // 测试"一"在不同上下文中的转换
    assert_eq!(chinese_numbers_to_primitive_numbers("一些"), "一些");
    assert_eq!(chinese_numbers_to_primitive_numbers("一样"), "一样");
    assert_eq!(chinese_numbers_to_primitive_numbers("一般"), "一般");
    assert_eq!(chinese_numbers_to_primitive_numbers("一直"), "一直");
    assert_eq!(chinese_numbers_to_primitive_numbers("一定"), "一定");
    assert_eq!(chinese_numbers_to_primitive_numbers("已经"), "已经");
    // 但"一个"应该转换，因为"个"是量词
    assert_eq!(chinese_numbers_to_primitive_numbers("一个人"), "1个人");
    assert_eq!(
        chinese_numbers_to_primitive_numbers("这本书有一百页"),
        "这本书有100页"
    );
}

#[test]
fn test_multiple_numbers() {
    // 测试包含多个数字的文本
    assert_eq!(
        chinese_numbers_to_primitive_numbers("五加十等于十五"),
        "5加10等于15"
    );
    assert_eq!(
        chinese_numbers_to_primitive_numbers("一百减五十等于五十"),
        "100减50等于50"
    );
}

#[test]
fn test_zero_and_special() {
    // 测试零和特殊情况
    assert_eq!(chinese_numbers_to_primitive_numbers("零"), "0");
    assert_eq!(chinese_numbers_to_primitive_numbers("一百零五"), "105");
    assert_eq!(
        chinese_numbers_to_primitive_numbers("今天零下五度"),
        "今天0下5度"
    );
}

#[test]
fn test_empty_and_no_numbers() {
    // 测试空字符串和没有数字的情况
    assert_eq!(chinese_numbers_to_primitive_numbers(""), "");
    assert_eq!(chinese_numbers_to_primitive_numbers("你好世界"), "你好世界");
    assert_eq!(
        chinese_numbers_to_primitive_numbers("没有数字的文本"),
        "没有数字的文本"
    );
}

#[test]
fn test_traditional_chinese() {
    // 测试繁体中文数字
    assert_eq!(chinese_numbers_to_primitive_numbers("壹"), "1");
    assert_eq!(chinese_numbers_to_primitive_numbers("叁拾伍"), "35");
}

#[test]
fn test_decimal_numbers() {
    // 测试小数转换（只有数字中间的"点"才转换）
    assert_eq!(chinese_numbers_to_primitive_numbers("三点一四"), "3.14");
    assert_eq!(chinese_numbers_to_primitive_numbers("零点五"), "0.5");
    assert_eq!(chinese_numbers_to_primitive_numbers("十点五"), "10.5");
    assert_eq!(
        chinese_numbers_to_primitive_numbers("一百二十三点四五"),
        "123.45"
    );
}

#[test]
fn test_dian_not_decimal() {
    // 测试非小数点的情况
    assert_eq!(chinese_numbers_to_primitive_numbers("三点钟"), "3点钟");
    assert_eq!(chinese_numbers_to_primitive_numbers("重点"), "重点");
    assert_eq!(chinese_numbers_to_primitive_numbers("点对点"), "点对点");
    assert_eq!(
        chinese_numbers_to_primitive_numbers("五点钟开会"),
        "5点钟开会"
    );
    // 注意："一点八"会转换成"1.8"因为后面有数字
    assert_eq!(chinese_numbers_to_primitive_numbers("一点八"), "1.8");
    // "一点八点二"会转换成"1.8.2"
    assert_eq!(chinese_numbers_to_primitive_numbers("一点八点二"), "1.8.2");
}

#[test]
fn test_decimal_mixed_text() {
    // 测试混合文本中的小数
    assert_eq!(
        chinese_numbers_to_primitive_numbers("版本一点八点二"),
        "版本1.8.2"
    );
    assert_eq!(
        chinese_numbers_to_primitive_numbers("温度三点五度和三点一五度"),
        "温度3.5度和3.15度"
    );
    assert_eq!(
        chinese_numbers_to_primitive_numbers("圆周率约等于三点一四一五九"),
        "圆周率约等于3.14159"
    );
}

#[test]
fn test_complex_decimal() {
    // 测试复杂小数场景
    assert_eq!(
        chinese_numbers_to_primitive_numbers("一千点零零一"),
        "1000.001"
    );
    assert_eq!(
        chinese_numbers_to_primitive_numbers("价格是一点五万元"),
        "价格是1.5万元"
    );
}

#[test]
fn test_non_standard_formats() {
    // 测试非标准格式的智能转换
    // "八六"会逐位转换为"86"
    assert_eq!(chinese_numbers_to_primitive_numbers("八六"), "86");
    // "叉八六杠六十四"会转换为"叉86杠64"
    assert_eq!(
        chinese_numbers_to_primitive_numbers("叉八六杠六十四"),
        "叉86杠64"
    );
    // "二十六十四"会被智能分割为"二十六"和"十四"
    assert_eq!(chinese_numbers_to_primitive_numbers("二十六十四"), "2614");
    // 完整的x86-64平台描述
    assert_eq!(
        chinese_numbers_to_primitive_numbers("主要针对的平台是叉八六杠六十四二十六十四，还有power P C64。"),
        "主要针对的平台是叉86杠642614，还有power P C64。"
    );
}
