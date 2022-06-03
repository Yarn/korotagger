
use std::fmt::Debug;
// use std::time::SystemTime;
use std::collections::HashSet;
use serde::Deserialize;
// use crate::DB;
// use crate::{ Stream, State, Offset };
use discord_lib::SendHandle;
#[allow(unused_imports)]
use discord_lib::discord::Snowflake;
use discord_lib::tokio::time::timeout;
use discord_lib::serde_json;
#[allow(unused_imports)]
use chrono::Utc;
use chrono::DateTime;
#[allow(unused_imports)]
use crate::i_love_youtube::get_stream_start_time;
#[allow(unused_imports)]
use crate::{from_i, to_i};
#[allow(unused_imports)]
use anyhow::{ Error, Context, anyhow };
use crate::util::Anyway;

// {
//   channels(
//     organizations:["Nijisanji"]
//     channel_id: ["UCV1xUwfM2v2oBtT3JNvic3w"]
//   ) {
//     items {
//       name {en}
//       platform_id
//     }
//   }
// }

const URL: &str = "https://api.chooks.app/v1/live";
const QUERY: &str = r#"
{
  live(
    exclude_organizations: ["Hololive"]
    # exclude_organizations: []
    # organizations: [
    #   "animare"
    #   "hanayori"
    #   "honeystrap"
    #   "noripro"
    #   "others"
    #   "react"
    #   "sugarlyric"
    #   "vapart"
    #   "vivid"
    #   "xencount"
    #   "idol-bu"
    #   "live"
    #   "voms"
    # ]
  ) {
    _id
    channel_id
    title
    time {
      #published
      #scheduled
      start
      #end
      #duration
    }
    # time {
    #   scheduled
    #   start
    # }
    # status
  }
}
"#;
const URL_B: &str = "https://api.chooks.app/koro";
const QUERY_B: &str = r#"
{
  live(
    exclude_organizations: []
  ) {
    _id
    channel_id
    title
    time {
      start
    }
  }
}
"#;

#[derive(Debug, Clone, Deserialize)]
struct Time {
    start: String,
}

#[derive(Debug, Clone, Deserialize)]
struct VideoObject {
    #[serde(rename = "_id")]
    id: String,
    channel_id: String,
    title: String,
    time: Time,
}

#[derive(Debug, Clone, Deserialize)]
struct ChooksLive {
    live: Vec<VideoObject>,
}

#[derive(Debug, Clone, Deserialize)]
struct ChooksResponse {
    data: ChooksLive,
}

fn print_deser_err(err: &serde_json::error::Error, body_str: &str) {
    println!("{}", err);
    
    if let Some(line) = body_str.lines().skip(err.line()-1).next() {
        let col = err.column();
        let start = ((col as isize) - 5).max(0) as usize;
        let end = (col + 500).min(line.len());
        
        fn find_char_boundary(s: &str, i: usize) -> usize {
            let mut bound = i;
            while !s.is_char_boundary(bound) {
                bound -= 1;
            }
            bound
        }
        let start = find_char_boundary(line, start);
        let end = find_char_boundary(line, end);
        
        let sub_str = &line[start..end];
        let arrow = "     ^";
        // dbg!(sub_str);
        println!("{}", sub_str);
        println!("{}", arrow);
    } else {
        println!("Invalid line number.");
    }
}

async fn http_req(url: &str, query: &str) -> Result<ChooksLive, Error> {
    
    
    use discord_lib::hyper;
    use discord_lib::hyper::{Body, Request};
    
    let query = query.replace("\n", "\\n");
    let query = query.replace("\"", "\\\"");
    let query = format!(r#"{{"operationName":null,"variables":{{}},"query":"{}"}}"#, query);
    // println!("{}", query);
    
    let req = Request::builder()
        .method("POST")
        .uri(url)
        .header("Content-Type", "application/json")
        .body(Body::from(query))
        .expect("request builder");
    
    let client = discord_lib::send_message::get_client()
        .anyway().context("get_client")?;
    
    let res = client.request(req).await.context("client request")?;
    
    let body = hyper::body::to_bytes(res).await.context("body to_bytes")?;
    
    let body_str: &str = std::str::from_utf8(&body).context("utf8 decode")?;
    // let a = hyper::body::to_bytes(body);
    
    let data: ChooksResponse = serde_json::from_str(body_str)
        .map_err(|err| {
            // println!("{:?}", body_str);
            print_deser_err(&err, body_str);
            err
        })
        .context("parse json")?;
    
    Ok(data.data)
}

use crate::auto_stream_live::GenericLive;
use crate::auto_stream_live::process_generic;
use crate::DState;

pub async fn get_live() -> Result<Vec<GenericLive>, Error> {
    
    let mut data = timeout(
        ::std::time::Duration::new(60, 0),
        http_req(URL, QUERY),
    ).await.context("timeout")??;
    let data_b = timeout(
        ::std::time::Duration::new(60, 0),
        http_req(URL_B, QUERY_B),
    ).await.context("timeout")??;
    
    data.live.extend(data_b.live);
    // ).await {
    //     Ok(x) => x,
    //     Err(err) => {
    //         dbg!("auto live timeout", err);
    //         delay_for(::std::time::Duration::new(60, 0)).await;
    //         continue;
    //     }
    // }.some_error("read body")?;
    
    let mut out: Vec<GenericLive> = Vec::new();
    
    for video_object in data.live {
        let start_time = DateTime::parse_from_rfc3339(&video_object.time.start).context("parse_from_rfc3339")?;
        let live = GenericLive {
            id: video_object.id,
            channel: video_object.channel_id,
            title: video_object.title,
            start_time: start_time,
        };
        out.push(live);
    }
    
    Ok(out)
}

pub async fn run_chooks(
    active: &mut HashSet<String>,
    send_handle: &SendHandle,
    pool: &sqlx::PgPool,
    d_state: DState,
) -> Result<(), Error> {
    
    let lives = get_live().await?;
    
    process_generic(
        &lives,
        active,
        send_handle,
        pool,
        d_state,
    ).await?;
    
    Ok(())
}
