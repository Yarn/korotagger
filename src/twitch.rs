
use anyhow::Context;

use serde::Deserialize;
use url::{ Url, Host };

use discord_lib::hyper::Uri;
use discord_lib::{ hyper, serde_json };
use hyper::{Body, Method, Request};
use discord_lib::bytes::buf::BufExt;

use chrono::{ DateTime, FixedOffset };

// https://www.twitch.tv/videos/1111111111?filter=archives&sort=time
pub fn extract_id(stream_url: &str) -> Option<String> {
    let url = Url::parse(stream_url).ok()?;
    
    let host = url.host()?;
    let host = match host {
        Host::Domain(host) => host,
        _ => return None,
    };
    
    if !&["www.twitch.tv", "twitch.tv"].contains(&host) {
        return None
    }
    
    let mut segs = url.path_segments()?;
    
    if let Some(seg) = segs.next() {
        if seg != "videos" {
            return None
        }
    }
    
    segs.next().map(|x| x.to_string())
}

#[derive(Debug, Deserialize)]
struct CactusTimestamp {
    created_at: String,
}

pub async fn get_start_time(video_id: &str) -> anyhow::Result<DateTime<FixedOffset>> {
    let client = discord_lib::send_message::get_client().unwrap();
    
    let path = format!("/timestamps/video?id={}", video_id);
    
    // https://gist.github.com/Decicus/ec4745e680e06cfff5b1fa0a53fcff72
    let uri = Uri::builder()
        .scheme("https")
        .authority("twitch-api-proxy.cactus.workers.dev")
        .path_and_query(path.as_str())
        .build()
        .unwrap();
    
    // dbg!(&uri);
    
    let req = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header("Host", "twitch-api-proxy.cactus.workers.dev")
        .body(Body::empty())
        .expect("request builder");
    
    // dbg!(&req);
    
    // let res = client.get(uri).await.context("client get")?;
    let res = client.request(req).await.context("client get")?;
    let body = hyper::body::to_bytes(res).await?;
    
    // let a = std::str::from_utf8(&body)?;
    // dbg!(a);
    
    let data: CactusTimestamp = serde_json::from_reader(body.reader())?;
    
    let created_at = DateTime::parse_from_rfc3339(&data.created_at)?;
    
    Ok(created_at)
}

#[cfg(test)]
mod tests {
    #[test]
    fn non_url() {
        assert_eq!(super::extract_id("a"), None);
        assert_eq!(super::extract_id("twitch"), None);
    }
    
    #[test]
    fn wrong_host() {
        assert_eq!(super::extract_id("http://www.youtube.com/watch?v=aaaaaa"), None);
        assert_eq!(super::extract_id("http://www.youtube.com/videos/1111111111"), None);
    }
    
    #[test]
    fn wrong_segs() {
        assert_eq!(super::extract_id("http://www.twitch.tv/"), None);
        assert_eq!(super::extract_id("http://www.twitch.tv/aaa/1111111111"), None);
    }
    
    #[test]
    fn correct_url() {
        assert_eq!(super::extract_id("http://www.twitch.tv/videos/1111111111"), Some("1111111111".to_owned()));
    }
}
