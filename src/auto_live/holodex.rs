
use anyhow::Context;

use serde::Deserialize;

// use discord_lib::hyper::Uri;
// use discord_lib::hyper;
// use hyper::{Body, Method, Request};

use chrono::{ DateTime, FixedOffset };

use std::collections::HashSet;
use crate::auto_stream_live::{ GenericLive, process_generic };

#[derive(Debug, Deserialize)]
struct Channel {
    id: String,
}

#[derive(Debug, Deserialize)]
struct Stream {
    id: String,
    channel: Channel,
    title: String,
    start_actual: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, Deserialize)]
#[serde(transparent)]
struct Response {
    streams: Vec<Stream>,
}

async fn get_streams(api_key: &str) -> anyhow::Result<Vec<Stream>> {
    // let client = discord_lib::send_message::get_client().unwrap();
    let client = reqwest::ClientBuilder::new()
        .timeout(::std::time::Duration::from_secs(10))
        .build()?;
    
    // https://holodex.net/api/v2/live?status=live
    // let uri = Uri::builder()
    //     .scheme("https")
    //     .authority("holodex.net")
    //     .path_and_query("/api/v2/live?status=live")
    //     .build()
    //     .unwrap();
    let url = "https://holodex.net/api/v2/live?status=live";
    
    // dbg!(&uri);
    
    // let req = Request::builder()
    let req = client.get(url)
        // .method(Method::GET)
        // .uri(uri)
        .header("Host", "holodex.net")
        .header("Content-Type", "application/json")
        .header("X-APIKEY", api_key)
        // .body(Body::empty())
        .build()
        .expect("request builder");
    
    // dbg!(&req);
    
    // dbg!();
    // let res = client.get(uri).await.context("client get")?;
    // let res = client.request(req).await.context("client get")?;
    let res = client.execute(req).await.context("client get")?;
    // let body = hyper::body::to_bytes(res).await?;
    let body: Vec<u8> = res.bytes().await?.to_vec();
    // dbg!();
    
    // let a = std::str::from_utf8(&body)?;
    // dbg!(a);
    
    let body_str: &str = std::str::from_utf8(&body)?;
    let data: Response = crate::util::parse_json_print_err(body_str)?;
    
    // let data: Response = serde_json::from_reader(body.reader())?;
    
    // let created_at = DateTime::parse_from_rfc3339(&data.created_at)?;
    
    // Ok(created_at)
    Ok(data.streams)
}

pub async fn auto_live_task(
    active: &mut HashSet<String>, pool: &sqlx::PgPool,
    states: &[crate::DiscordState],
    channel_cache: &mut crate::auto_stream_live::ChannelCache,
    api_key: &str
) -> anyhow::Result<()> {
    let streams = get_streams(api_key).await.context("get_streams")?;
    
    let generic = streams.into_iter()
        .filter_map(|stream| {
            let live = GenericLive {
                id: stream.id,
                channel: stream.channel.id,
                title: stream.title,
                start_time: stream.start_actual
                    .map(|d| { d })?,
                    // .unwrap_or_else(|| { Utc::now().into() }),
            };
            Some(live)
        })
        .collect::<Vec<_>>();
    
    process_generic(
        &generic,
        active,
        pool,
        states,
        channel_cache,
    ).await.context("generic")?;
    
    Ok(())
}
