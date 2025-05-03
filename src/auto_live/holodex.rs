
use anyhow::Context;

use serde::Deserialize;

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
    
    let url = "https://holodex.net/api/v2/live?status=live";
    
    // dbg!(&uri);
    
    let req = client.get(url)
        // .method(Method::GET)
        // .uri(uri)
        .header("Host", "holodex.net")
        .header("Content-Type", "application/json")
        .header("X-APIKEY", api_key)
        .build()
        .expect("request builder");
    
    let res = client.execute(req).await.context("client get")?;
    let body: Vec<u8> = res.bytes().await?.to_vec();
    
    let body_str: &str = std::str::from_utf8(&body)?;
    let data: Response = crate::util::parse_json_print_err(body_str)?;
    
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
