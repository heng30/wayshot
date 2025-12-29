use srtmp::client::RtmpClientConfig;

#[test]
fn test_parse_url_with_query_params() {
    let config =
        RtmpClientConfig::parse_url("rtmp://localhost:1935/live/stream?key=value&token=abc")
            .unwrap();

    assert_eq!(config.rtmp_url, "rtmp://localhost:1935");
    assert_eq!(config.app, "live");
    assert_eq!(config.stream_key, "stream");
    assert_eq!(config.query_params, "key=value&token=abc");
    assert_eq!(config.port, 1935);
}

#[test]
fn test_parse_url_without_query_params() {
    let config = RtmpClientConfig::parse_url("rtmp://localhost:1935/live/stream").unwrap();

    assert_eq!(config.rtmp_url, "rtmp://localhost:1935");
    assert_eq!(config.app, "live");
    assert_eq!(config.stream_key, "stream");
    assert_eq!(config.query_params, "");
    assert_eq!(config.port, 1935);
}

#[test]
fn test_parse_url_without_port() {
    let config = RtmpClientConfig::parse_url("rtmp://localhost/live/stream").unwrap();

    assert_eq!(config.rtmp_url, "rtmp://localhost");
    assert_eq!(config.app, "live");
    assert_eq!(config.stream_key, "stream");
    assert_eq!(config.port, 1935); // Default port
}

#[test]
fn test_parse_url_with_custom_port() {
    let config = RtmpClientConfig::parse_url("rtmp://example.com:8080/live/stream").unwrap();

    assert_eq!(config.rtmp_url, "rtmp://example.com:8080");
    assert_eq!(config.port, 8080);
}

#[test]
fn test_parse_url_with_empty_query_params() {
    let config = RtmpClientConfig::parse_url("rtmp://localhost:1935/live/stream?").unwrap();

    assert_eq!(config.app, "live");
    assert_eq!(config.stream_key, "stream");
    assert_eq!(config.query_params, "");
}

#[test]
fn test_parse_url_with_ip_address() {
    let config = RtmpClientConfig::parse_url("rtmp://192.168.1.100:1935/live/stream").unwrap();

    assert_eq!(config.rtmp_url, "rtmp://192.168.1.100:1935");
    assert_eq!(config.app, "live");
    assert_eq!(config.stream_key, "stream");
}

#[test]
fn test_parse_url_invalid_no_scheme() {
    let result = RtmpClientConfig::parse_url("http://localhost:1935/live/stream");
    assert!(result.is_err());
}

#[test]
fn test_parse_url_invalid_no_path() {
    let result = RtmpClientConfig::parse_url("rtmp://localhost:1935");
    assert!(result.is_err());
}

#[test]
fn test_parse_url_invalid_no_app() {
    let result = RtmpClientConfig::parse_url("rtmp://localhost:1935/");
    assert!(result.is_err());
}

#[test]
fn test_from_url_convenience_method() {
    let config = RtmpClientConfig::from_url("rtmp://localhost:1935/live/stream?key=value")
        .unwrap();

    assert_eq!(config.app, "live");
    assert_eq!(config.stream_key, "stream");
    assert_eq!(config.query_params, "key=value");
}

#[test]
fn test_build_stream_name_without_params() {
    let config = RtmpClientConfig::new(
        "rtmp://localhost:1935".to_string(),
        "live".to_string(),
        "stream".to_string(),
    );

    assert_eq!(config.build_stream_name(), "stream");
}

#[test]
fn test_build_stream_name_with_params() {
    let config = RtmpClientConfig::new(
        "rtmp://localhost:1935".to_string(),
        "live".to_string(),
        "stream".to_string(),
    )
    .with_query_params("key=value&token=abc".to_string());

    assert_eq!(config.build_stream_name(), "stream?key=value&token=abc");
}

#[test]
fn test_build_stream_name_with_single_param() {
    let config = RtmpClientConfig::new(
        "rtmp://localhost:1935".to_string(),
        "live".to_string(),
        "stream".to_string(),
    )
    .with_query_params("token=xyz123".to_string());

    assert_eq!(config.build_stream_name(), "stream?token=xyz123");
}

#[test]
fn test_backward_compatibility_new_method() {
    // Test that the existing new() method still works
    let config = RtmpClientConfig::new(
        "rtmp://localhost:1935".to_string(),
        "live".to_string(),
        "stream".to_string(),
    );

    assert_eq!(config.rtmp_url, "rtmp://localhost:1935");
    assert_eq!(config.app, "live");
    assert_eq!(config.stream_key, "stream");
    assert_eq!(config.query_params, ""); // Default empty
    assert_eq!(config.port, 1935); // Default
    assert_eq!(config.build_stream_name(), "stream"); // No params
}

#[test]
fn test_with_query_params_setter() {
    let config = RtmpClientConfig::new(
        "rtmp://localhost:1935".to_string(),
        "live".to_string(),
        "stream".to_string(),
    )
    .with_query_params("key=value".to_string());

    assert_eq!(config.query_params, "key=value");
    assert_eq!(config.build_stream_name(), "stream?key=value");
}

#[test]
fn test_parse_url_with_complex_query_params() {
    let config = RtmpClientConfig::parse_url(
        "rtmp://localhost:1935/live/stream?key=value&token=abc&user_id=123&active=true",
    )
    .unwrap();

    assert_eq!(
        config.query_params,
        "key=value&token=abc&user_id=123&active=true"
    );
    assert_eq!(
        config.build_stream_name(),
        "stream?key=value&token=abc&user_id=123&active=true"
    );
}

#[test]
fn test_parse_url_preserves_url_encoded_chars() {
    let config = RtmpClientConfig::parse_url(
        "rtmp://localhost:1935/live/stream?key=hello%20world&token=abc%3Ddef",
    )
    .unwrap();

    assert_eq!(config.query_params, "key=hello%20world&token=abc%3Ddef");
    assert_eq!(config.build_stream_name(), "stream?key=hello%20world&token=abc%3Ddef");
}
