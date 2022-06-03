
use async_trait::async_trait;

use super::{ Handler, HandlerResult, HandlerResponse, HandlerError, Command };
use discord_lib::discord::Message;
// use crate::DB;

use std::sync::Arc;
use discord_lib::tokio::sync::RwLock;
use discord_lib::tokio::time::delay_for;
#[allow(unused_imports)]
use discord_lib::tokio::{self, task};
use std::collections::BTreeMap;
use chrono::{ DateTime, FixedOffset, Utc };

type DateTimeF = DateTime<FixedOffset>;

fn spawn_cleanup_task(history_lock: Arc<RwLock<History>>) {
    task::spawn(async move {
        loop {
            delay_for(::std::time::Duration::new(12*60*60, 0)).await;
            
            let now: DateTimeF = Utc::now().into();
            {
                let history: &mut History = &mut *history_lock.write().await;
                
                for messages in history.messages.values_mut() {
                    messages.retain(|msg_time| {
                        (now - *msg_time).num_days() < 3
                    })
                }
            }
        }
    });
}

#[derive(Debug)]
struct History {
    messages: BTreeMap<u64, Vec<DateTimeF>>,
}

#[derive(Debug)]
pub struct HistogramHandler {
    history: Arc<RwLock<History>>,
}

impl HistogramHandler {
    pub fn new() -> Self {
        let history = History { messages: BTreeMap::new() };
        let lock = Arc::new(RwLock::new(history));
        spawn_cleanup_task(lock.clone());
        HistogramHandler {
            history: lock,
        }
    }
    
    pub fn rebuild_handler(&self, auth: String) -> RebuildHandler {
        RebuildHandler {
            history: self.history.clone(),
            auth: auth,
        }
    }
}

#[async_trait]
impl Handler for HistogramHandler {
    async fn handle_command(&self, args: &[&str], msg: &Message) -> HandlerResult {
        
        let channel_id_arg = args.get(0).unwrap_or(&"_");
        let max_minutes_arg: Option<i64> = match args.get(1) {
            Some(x) => {
                Some(x.parse().map_err(|_err| {
                    HandlerError::with_message("Invalid num minutes".into())
                })?)
            }
            None => Some(6*60),
            // None => None,
        };
        
        let channel_id: u64 = if *channel_id_arg == "_" {
            msg.channel_id.0
        } else {
            channel_id_arg.parse().map_err(|_err| {
                HandlerError::with_message("Invalid channel id".into())
            })?
        };
        
        let (start_time, stream_name): (DateTimeF, String) = {
            let data = DB.borrow_data().unwrap();
            
            let stream_name = match data.current_stream.get(&channel_id) {
                Some(s) => s,
                None => {
                    return Err(HandlerError::with_message("No active stream".into()))
                }
            };
            
            match data.streams.get(stream_name) {
                Some(stream) => {
                    (stream.start_time, stream_name.clone())
                }
                None => {
                    return Err(HandlerError::with_message("No active stream".into()))
                }
            }
        };
        
        let mut grouped: BTreeMap<i64, u64> = BTreeMap::new();
        {
            let history: &History = &*self.history.read().await;
            let messages: &[DateTimeF] = match history.messages.get(&channel_id) {
                Some(messages) => messages,
                None => &[],
            };
            // dbg!(history);
            
            
            
            for msg_time in messages {
                let minutes = (*msg_time - start_time).num_minutes();
                *grouped.entry(minutes).or_insert(0) += 1;
            }
        }
        
        let grouped_min_minute = *grouped.keys().min().unwrap_or(&0);
        let grouped_min_minute = grouped_min_minute.max(0);
        let mut grouped_max_minute = *grouped.keys().max().unwrap_or(&0);
        let max_count = *grouped.values().max().unwrap_or(&0);
        let line_width = 40;
        
        if let Some(max_minute) = max_minutes_arg {
            grouped_max_minute = grouped_max_minute.min(max_minute);
        }
        
        let mut out: String = "".into();
        for min in grouped_min_minute..=grouped_max_minute {
            let count = *grouped.get(&min).unwrap_or(&0);
            // dbg!(min, count, max_count);
            // if count != 0 {
            // let mut line = String::new();
            // if count != 0 {
            let bar = (count as f32 / max_count as f32) * line_width as f32;
            let bar = bar.ceil() as u64;
            let bar = bar.min(count);
            
            out.push_str(&format!("[`{:>4}m ({:>3}) ", min, count));
            for _ in 0..bar {
                out.push('+');
            }
            out.push_str(&format!("`]({}&t={}m)", stream_name, min));
            // }
            out.push('\n');
            // else {
            //     out.push("\n");
            // }
        }
        // out.push_str("```");
        
        let res = HandlerResponse::wrapped_embed(None, &out);
        Ok(res)
        // Ok(().into())
    }
    
    async fn handle_message(&self, msg: &Message) -> HandlerResult {
        
        let channel_id: u64 = msg.channel_id.0;
        let now: DateTimeF = Utc::now().into();
        
        {
            let history: &mut History = &mut *self.history.write().await;
            
            let channel_history = history.messages
                .entry(channel_id)
                .or_insert_with(|| Vec::new());
            
            channel_history.push(now);
        }
        
        Ok(().into())
    }
}

async fn get_messages(auth: &str, channel_id: u64, before: u64) -> Result<Vec<Message>, ()> {
    use discord_lib::hyper;
    use hyper::Body;
    use hyper::Request;
    // use hyper::Client;
    use hyper::header::{AUTHORIZATION, CONTENT_TYPE};
    use hyper::http::StatusCode;
    
    let client = discord_lib::send_message::get_client().unwrap();
    
    let url = &format!("https://discordapp.com/api/v6/channels/{}/messages?before={}", channel_id, before);
    // let body: Body = format!(r#"{{"before": "{}", "limit": 90}}"#, before).into();
    // dbg!(url, &body);
    
    let req = Request::builder()
        .method("GET")
        .uri(url)
        
        .header(AUTHORIZATION, auth)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::empty())
        // .body(body)
        .map_err(|err| {
            dbg!(err);
            ()
        })?;
    
    let res = client.request(req).await.map_err(|err| {
        dbg!(err);
        ()
    })?;
    
    let (parts, body) = res.into_parts();
    
    let body = {
        let b: Vec<u8> = hyper::body::to_bytes(body).await.map_err(|err| {
            dbg!(err);
            ()
        })?.to_vec();
        
        let body = String::from_utf8(b).map_err(|err| {
            dbg!(err);
            ()
        })?;
        
        body
    };
    
    if parts.status != StatusCode::OK {
        dbg!(body, parts.status);
        return Err(());
    }
    
    use discord_lib::serde_json;
    let messages: Vec<Message> = serde_json::from_str(&body).map_err(|err| {
        dbg!(err);
        ()
    })?;
    
    Ok(messages)
    // Ok(())
    // panic!()
}

#[derive(Debug)]
pub struct RebuildHandler {
    history: Arc<RwLock<History>>,
    auth: String,
}

#[async_trait]
impl Handler for RebuildHandler {
    async fn handle_command_b(&self, command: Command<'_>) -> HandlerResult {
        command.require_admin().await?;
        
        let args = command.args;
        let msg = command.message;
        
        let channel_id_arg = args.get(0).unwrap_or(&"_");
        let num_messages: u64 = args.get(1)
            .map(|x| {
                x.parse().map_err(|_err| {
                    HandlerError::with_message("Invalid num messages".into())
                })
            }).unwrap_or(Ok(1000))?;
        
        let channel_id: u64 = if *channel_id_arg == "_" {
            msg.channel_id.0
        } else {
            channel_id_arg.parse().map_err(|_err| {
                HandlerError::with_message("Invalid channel id".into())
            })?
        };
        
        let mut msg_id: u64 = msg.id.0;
        
        let mut messages = get_messages(&self.auth, channel_id, msg_id).await.map_err(|_| {
            HandlerError::with_message("Could not receive messages.".into())
        })?;
        
        if messages.len() == 0 {
            return Err(HandlerError::with_message("No messages received.".into()))
        }
        
        {
            let history: &mut History = &mut *self.history.write().await;
            history.messages.entry(channel_id).or_insert_with(|| Vec::new()).clear();
        }
        
        let mut processed = 0;
        loop {
            if let Some(msg) = messages.last() {
                msg_id = msg.id.0;
            } else {
                break
            }
            
            {
                let history: &mut History = &mut *self.history.write().await;
                let message_times: &mut Vec<DateTimeF> = match history.messages.get_mut(&channel_id) {
                    Some(messages) => messages,
                    None => break,
                };
                
                for msg in messages.iter() {
                    let timestamp = DateTime::parse_from_rfc3339(&msg.timestamp_str)
                        .map_err(|err| {
                            dbg!(err);
                            HandlerError::with_message("Bad timestamp from discord api.".into())
                        })?;
                    
                    message_times.push(timestamp);
                    
                    processed += 1;
                }
            }
            
            if processed > num_messages {
                break
            }
            
            messages = get_messages(&self.auth, channel_id, msg_id).await.map_err(|_| {
                HandlerError::with_message("Failed receiving more messages.".into())
            })?;
        }
        
        Ok("History rebuilt".into())
    }
}
