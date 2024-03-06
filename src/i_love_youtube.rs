
// use std::str::FromStr;
// use std::collections::BTreeMap;

use serde::Deserialize;
// use serde::de::IgnoredAny;
use chrono::{ DateTime, FixedOffset };

// use scraper::Html;
// use scraper::Selector;
// use bytes::buf::BufExt;

// use discord_lib::hyper::Uri;
// use discord_lib::{ hyper, serde_json, bytes };
// use discord_lib::{ serde_json };


use crate::auto_stream_live::{ SomeError, ToSomeError };

lazy_static::lazy_static!{
    pub static ref YT_API_KEY: String = {
        std::env::var("yt_api_key").expect("environment variable yt_api_key not set")
    };
}

#[derive(Debug, Deserialize)]
struct TimeLiveStreamDetails {
    #[serde(alias = "actualStartTime")]
    actual_start_time: String,
}

#[derive(Debug, Deserialize)]
struct TimeItem {
    #[serde(alias = "liveStreamingDetails")]
    live_stream_details: TimeLiveStreamDetails,
}

#[derive(Debug, Deserialize)]
struct TimeVideosList {
    items: Vec<TimeItem>,
}

#[derive(Debug, Deserialize)]
struct LiveStreamDetails {
    #[serde(alias = "actualStartTime")]
    actual_start_time: Option<String>,
    #[serde(alias = "scheduledStartTime")]
    scheduled_start_time: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct LocalizationInfo {
    title: String,
    // description: String,
}

#[derive(Debug, Deserialize)]
struct Snippet {
    // localized: LocalizationInfo,
    title: String,
}

#[derive(Debug, Deserialize)]
struct Item {
    #[serde(alias = "liveStreamingDetails")]
    live_stream_details: LiveStreamDetails,
    snippet: Snippet,
    // localizations: BTreeMap<String, LocalizationInfo>,
}

#[derive(Debug, Deserialize)]
struct VideosList {
    items: Vec<Item>,
}

#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub best_start_time: DateTime<FixedOffset>,
    pub title: String,
}

// https://www.youtube.com/watch?v=aaaaaa
// http://www.youtube.com/watch?v=aaaaaa
// https://youtu.be/aaaaaa
// https://www.youtube.com/embed/aaaaaa?enablejsapi=1
pub fn extract_id(stream_url: &str) -> Option<&str> {
    let stream_url = if stream_url.starts_with("https://") {
        stream_url.trim_start_matches("https://")
    } else if stream_url.starts_with("http://") {
        stream_url.trim_start_matches("http://")
    } else {
        return None;
    };
    
    if stream_url.starts_with("www.youtube.com/watch?v=") {
        let mut part = stream_url.trim_start_matches("www.youtube.com/watch?v=");
        part = part.splitn(2, '&').next()?;
        part = part.trim_end_matches('/');
        Some(part)
    } else if stream_url.starts_with("youtube.com/watch?v=") {
        let mut part = stream_url.trim_start_matches("youtube.com/watch?v=");
        part = part.splitn(2, '&').next()?;
        part = part.trim_end_matches('/');
        Some(part)
    } else if stream_url.starts_with("www.youtube.com/embed/") {
        let mut part = stream_url.trim_start_matches("www.youtube.com/embed/");
        part = part.splitn(2, '?').next()?;
        part = part.trim_end_matches('/');
        Some(part)
    } else if stream_url.starts_with("youtu.be/") {
        let mut part = stream_url.trim_start_matches("youtu.be/");
        part = part.splitn(2, '?').next()?;
        part = part.trim_end_matches('/');
        Some(part)
    } else {
        None
    }
}


pub async fn get_stream_info(video_id: &str) -> Result<VideoInfo, anyhow::Error> {
    // let client = discord_lib::send_message::get_client().unwrap();
    let client = reqwest::ClientBuilder::new()
        .timeout(::std::time::Duration::from_secs(10))
        .build()?;
    
    let path = format!(
        "/youtube/v3/videos?part=liveStreamingDetails,snippet&id={}&key={}",
        video_id,
        &*YT_API_KEY,
    );
    
    // let uri = Uri::builder()
    //     .scheme("https")
    //     .authority("www.googleapis.com")
    //     .path_and_query(path.as_str())
    //     .build()
    //     .unwrap();
    let url = format!("https://www.googleapis.com{}", path);
    
    // let res = client.get(uri).await?;
    // let body = hyper::body::to_bytes(res).await?;
    let req = client.get(url).build().expect("request builder");
    let res = client.execute(req).await?;
    let body = res.bytes().await?.to_vec();
    
    // let x = std::str::from_utf8(&body);
    // dbg!(x);
    
    // let mut data: VideosList = serde_json::from_reader(body.reader())?;
    let mut data: VideosList = serde_json::from_slice(&body)?;
    
    let ref video_info: Item = data.items
        .pop().ok_or_else(|| anyhow::anyhow!("No stream found"))?;
    
    if data.items.len() != 0 {
        dbg!(&data.items);
    }
    
    let ref details: LiveStreamDetails = video_info.live_stream_details;
    
    let start_time = details.actual_start_time.as_ref().or(details.scheduled_start_time.as_ref());
    // let start_time = DateTime::parse_from_rfc3339(start_time)?;
    let start_time = match start_time {
        Some(t) => DateTime::parse_from_rfc3339(t)?,
        None => return Err(anyhow::anyhow!("no start time")),
    };
    
    // 2020-05-29T09:02:48.800000Z
    
    // Ok(start_time)
    
    let video_info = VideoInfo {
        best_start_time: start_time,
        title: video_info.snippet.title.clone(),
    };
    
    // Err(anyhow::anyhow!(""))
    // panic!()
    Ok(video_info)
}

pub async fn get_stream_start_time(video_id: &str) -> Result<DateTime<FixedOffset>, SomeError> {
    
    // curl \
    //     'https://www.googleapis.com/youtube/v3/videos?part=id%2C%20liveStreamingDetails%2C%20contentDetails&id=LDqvVFHCt2g&key=[YOUR_API_KEY]' \
    //     --header 'Authorization: Bearer [YOUR_ACCESS_TOKEN]' \
    //     --header 'Accept: application/json' \
    //     --compressed
    
    
    // let client = discord_lib::send_message::get_client().unwrap();
    let client = reqwest::ClientBuilder::new()
        .timeout(::std::time::Duration::from_secs(10))
        .build()
        .unwrap();
        // .some_error?;
    
    let path = format!(
        "/youtube/v3/videos?part=liveStreamingDetails&id={}&key={}",
        video_id,
        &*YT_API_KEY,
    );
    
    // let uri = Uri::builder()
    //     .scheme("https")
    //     .authority("www.googleapis.com")
    //     .path_and_query(path.as_str())
    //     .build()
    //     .unwrap();
    let url = format!("https://www.googleapis.com{}", path);
    
    let req = client.get(url)
        .build()
        .expect("request builder");
    
    // let res = client.get(Uri::from_str(url).some_error()?).await.some_error()?;
    // let res = client.get(uri).await.some_error("client get")?;
    let res = client.execute(req).await.some_error("client get")?;
    
    // let a = hyper::body::to_bytes(res).await.unwrap();
    // println!("\n{}\n{}", path, String::from_utf8(a.to_vec()).unwrap());
    // panic!();
    
    // let body = hyper::body::to_bytes(res).await.some_error("to_bytes body")?;
    let body = res.bytes().await.some_error("to_bytes body")?.to_vec();
    
    // use discord_lib::hyper::body::Buf;
    // println!("{:?}", std::str::from_utf8(body.bytes()));
    // panic!();
    // let data: TimeVideosList = serde_json::from_reader(body.reader()).some_error("parse body")?;
    let data: TimeVideosList = serde_json::from_slice(&body).some_error("parse body")?;
    
    let ref details: TimeLiveStreamDetails = data.items
        .get(0).ok_or_else(|| SomeError::msg("No stream found"))?.live_stream_details;
    
    let ref start_time = details.actual_start_time;
    let start_time = DateTime::parse_from_rfc3339(start_time).some_error("parse_from_rfc3339")?;
    
    // 2020-05-29T09:02:48.800000Z
    
    Ok(start_time)
    
    // let body: Vec<u8> = hyper::body::to_bytes(res).await.some_error()?.to_vec();
    // let bytes: Vec<u8> = hyper::body::to_bytes(res).await.some_error();
    
    // let body = String::from_utf8(body).some_error()?;
    
    // let html = Html::parse_document(&body);
    
    // Ok(())
    
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extract_id() {
        assert_eq!(None, extract_id("a"));
        assert_eq!(Some("x"), extract_id("https://www.youtube.com/watch?v=x"));
        assert_eq!(Some("x"), extract_id("https://www.youtube.com/watch?v=x/"));
        assert_eq!(Some("x"), extract_id("https://www.youtube.com/watch?v=x&a=b/"));
        assert_eq!(Some("x"), extract_id("https://www.youtube.com/watch?v=x"));
        
        assert_eq!(Some("x"), extract_id("http://www.youtube.com/watch?v=x"));
        
        assert_eq!(Some("x"), extract_id("https://www.youtube.com/embed/x"));
        assert_eq!(Some("x"), extract_id("https://www.youtube.com/embed/x?a_b"));
        assert_eq!(Some("x"), extract_id("https://www.youtube.com/embed/x/"));
        assert_eq!(Some("x"), extract_id("https://www.youtube.com/embed/x/?a_b"));
        
        assert_eq!(Some("x"), extract_id("https://youtu.be/x"));
        assert_eq!(Some("x"), extract_id("https://youtu.be/x/"));
        assert_eq!(Some("x"), extract_id("https://youtu.be/x?a_b"));
        
        assert_eq!(Some("x"), extract_id("https://www.youtube.com/embed/x"));
        assert_eq!(Some("x"), extract_id("https://www.youtube.com/embed/x?a_b"));
        assert_eq!(Some("x"), extract_id("https://www.youtube.com/embed/x/"));
    }
}
