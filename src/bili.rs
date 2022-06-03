
// https://www.bilibili.com/video/---
// https://www.bilibili.com/video/---?t=1m25s
pub fn extract_id(stream_url: &str) -> Option<&str> {
    let stream_url = if stream_url.starts_with("https://") {
        stream_url.trim_start_matches("https://")
    } else if stream_url.starts_with("http://") {
        stream_url.trim_start_matches("http://")
    } else {
        return None;
    };
    
    if stream_url.starts_with("www.bilibili.com/video/") {
        let mut part = stream_url.trim_start_matches("www.bilibili.com/video/");
        part = part.splitn(2, '?').next()?;
        part = part.trim_end_matches('/');
        Some(part)
    } else {
        None
    }
}
