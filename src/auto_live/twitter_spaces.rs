
use anyhow::Context;

use serde::Deserialize;
use url::{ Url, Host };

// use discord_lib::hyper::Uri;
// use discord_lib::{ hyper, serde_json };
// use discord_lib::{ serde_json };
// use hyper::{Body, Method, Request};
// use discord_lib::bytes::buf::BufExt;

use chrono::{ DateTime, FixedOffset, Utc, TimeZone };

// const PATH_BEFORE: &str = r#"/i/api/graphql/ZAFnfXaKTWDvv_dh_dEs5w/AudioSpaceById?variables={"id":""#;
// const PATH_AFTER: &str = r#"","isMetatagsQuery":true,"withSuperFollowsUserFields":true,"withDownvotePerspective":false,"withReactionsMetadata":false,"withReactionsPerspective":false,"withSuperFollowsTweetFields":true,"withReplays":true}&features={"dont_mention_me_view_api_enabled":true,"interactive_text_enabled":true,"responsive_web_uc_gql_enabled":false,"vibe_tweet_context_enabled":false,"responsive_web_edit_tweet_api_enabled":false,"standardized_nudges_misinfo":false,"responsive_web_enhance_cards_enabled":false}"#;
const PATH_BEFORE: &str = r#"/i/api/graphql/ZAFnfXaKTWDvv_dh_dEs5w/AudioSpaceById?variables=%7B%22id%22%3A%22"#;
const PATH_AFTER: &str = r#"%22%2C%22isMetatagsQuery%22%3Atrue%2C%22withSuperFollowsUserFields%22%3Atrue%2C%22withDownvotePerspective%22%3Afalse%2C%22withReactionsMetadata%22%3Afalse%2C%22withReactionsPerspective%22%3Afalse%2C%22withSuperFollowsTweetFields%22%3Atrue%2C%22withReplays%22%3Atrue%7D&features=%7B%22dont_mention_me_view_api_enabled%22%3Atrue%2C%22interactive_text_enabled%22%3Atrue%2C%22responsive_web_uc_gql_enabled%22%3Afalse%2C%22vibe_tweet_context_enabled%22%3Afalse%2C%22responsive_web_edit_tweet_api_enabled%22%3Afalse%2C%22standardized_nudges_misinfo%22%3Afalse%2C%22responsive_web_enhance_cards_enabled%22%3Afalse%7D"#;

#[derive(Debug, Deserialize)]
struct GuestToken {
    guest_token: String,
}

#[derive(Debug, Deserialize)]
struct Metadata {
    created_at: i64,
}

#[derive(Debug, Deserialize)]
struct AudioSpace {
    metadata: Metadata,
}

#[derive(Debug, Deserialize)]
struct Data {
    #[serde(rename = "audioSpace")]
    audio_space: AudioSpace,
}

#[derive(Debug, Deserialize)]
struct SpaceData {
    data: Data,
}

pub fn extract_id(stream_url: &str) -> Option<String> {
    let url = Url::parse(stream_url).ok()?;
    
    let host = url.host()?;
    let host = match host {
        Host::Domain(host) => host,
        _ => return None,
    };
    
    if !&["twitter.com"].contains(&host) {
        return None
    }
    
    let mut segs = url.path_segments()?;
    
    if let Some(seg) = segs.next() {
        if seg != "i" {
            return None
        }
    }
    if let Some(seg) = segs.next() {
        if seg != "spaces" {
            return None
        }
    }
    
    segs.next().map(|x| x.to_string())
}

async fn get_guest_token(client: &reqwest::Client) -> anyhow::Result<String> {
    // let client = discord_lib::send_message::get_client().unwrap();
    // let client = reqwest::ClientBuilder::new()
    //     .build()?;
    
    // let path = "/1.1/guest/activate.json";
    
    
    // let uri = Uri::builder()
    //     .scheme("https")
    //     .authority("api.twitter.com")
    //     .path_and_query(path)
    //     .build()
    //     .unwrap();
    
    
    // let req = Request::builder()
    let req = client.post("https://api.twitter.com/1.1/guest/activate.json")
        // .method(Method::POST)
        // .uri(uri)
        .header("Host", "api.twitter.com")
        .header("authorization", "Bearer AAAAAAAAAAAAAAAAAAAAANRILgAAAAAAnNwIzUejRCOuH5E6I8xnZz4puTs=1Zv7ttfk8LF81IUq16cHjhLTvJu4FA33AGWWjCpTnA")
        // .body(Body::empty())
        .build()
        .expect("request builder");
    
    let res = client.execute(req).await.context("client get")?;
    
    // let body = hyper::body::to_bytes(res).await?;
    // let data: GuestToken = serde_json::from_reader(body.reader())?;
    let body: Vec<u8> = res.bytes().await?.to_vec();
    let data: GuestToken = serde_json::from_slice(&body)?;
    
    Ok(data.guest_token)
}

pub async fn get_start_time(video_id: &str) -> anyhow::Result<DateTime<FixedOffset>> {
    
    // let client = discord_lib::send_message::get_client().unwrap();
    let client = reqwest::ClientBuilder::new()
        .timeout(::std::time::Duration::from_secs(10))
        .build()?;
    
    let guest_token = get_guest_token(&client).await?;
    
    let path = format!("{}{}{}", PATH_BEFORE, video_id, PATH_AFTER);
    
    // let uri = Uri::builder()
    //     .scheme("https")
    //     .authority("twitter.com")
    //     .path_and_query(path.as_str())
    //     .build()
    //     .unwrap();
    let url = format!("https://twitter.com{}", path);
    
    // let req = Request::builder()
    let req = client.get(url)
        // .method(Method::GET)
        // .uri(uri)
        .header("Host", "twitter.com")
        .header("authorization", "Bearer AAAAAAAAAAAAAAAAAAAAANRILgAAAAAAnNwIzUejRCOuH5E6I8xnZz4puTs=1Zv7ttfk8LF81IUq16cHjhLTvJu4FA33AGWWjCpTnA")
        .header("x-guest-token", guest_token)
        // .body(Body::empty())
        .build()
        .expect("request builder");
    
    // let res = client.request(req).await.context("client get")?;
    let res = client.execute(req).await.context("client get")?;
    // let body = hyper::body::to_bytes(res).await?;
    let body: Vec<u8> = res.bytes().await?.to_vec();
    
    // let data: SpaceData = serde_json::from_reader(body.reader())?;
    let data: SpaceData = serde_json::from_slice(&body)?;
    
    let secs = data.data.audio_space.metadata.created_at / 1000;
    let created_at = Utc.timestamp_opt(secs, 0);
    
    Ok(created_at.single().ok_or_else(|| anyhow::anyhow!("ambiguos or invalid timestamp {}", secs))?.into())
}

#[cfg(test)]
mod tests {
    #[test]
    fn non_url() {
        assert_eq!(super::extract_id("a"), None);
        assert_eq!(super::extract_id("twitter.com"), None);
    }
    
    #[test]
    fn wrong_host() {
        assert_eq!(super::extract_id("http://www.youtube.com/watch?v=aaaaaa"), None);
        assert_eq!(super::extract_id("http://www.youtube.com/i/spaces/11111"), None);
    }
    
    #[test]
    fn wrong_segs() {
        assert_eq!(super::extract_id("http://twitter.com/"), None);
        assert_eq!(super::extract_id("http://twitter.com/i/aa/1111111111"), None);
    }
    
    #[test]
    fn correct_url() {
        assert_eq!(super::extract_id("https://twitter.com/i/spaces/111"), Some("111".to_owned()));
    }
}
