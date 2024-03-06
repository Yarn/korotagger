
use std::fmt::Debug;
// use std::time::SystemTime;
use std::collections::HashSet;
// use serde::Deserialize;
// use crate::DB;
// use crate::{ Stream, State, Offset };
use discord_lib::SendHandle;
use discord_lib::discord::Snowflake;
// use discord_lib::tokio::time::timeout;
#[allow(unused_imports)]
use chrono::Utc;
#[allow(unused_imports)]
use chrono::DateTime;
#[allow(unused_imports)]
use crate::i_love_youtube::get_stream_start_time;
#[allow(unused_imports)]
use crate::{from_i, to_i};
use crate::DateTimeF;
use crate::DState;
use anyhow::Context;
use anyhow::Error;

#[derive(Debug)]
#[allow(dead_code)]
pub struct SomeError {
    desc: String,
}

impl SomeError {
    fn new<T: Debug>(err: T, msg: &str) -> Self {
        SomeError {
            desc: format!("{} {:?}", msg, err),
        }
    }
    
    pub fn msg<T: Into<String>>(msg: T) -> Self {
        SomeError {
            desc: msg.into(),
        }
    }
}

// impl Into<SomeError> for &std::fmt::Debug {
//     fn into(err: T) -> SomeError {
//         SomeError {
//             desc: format!("{:?}", err),
//         }
//     }
// }
// impl<T: std::fmt::Debug> From<T> for SomeError {
//     fn from(err: T) -> SomeError {
//         SomeError {
//             desc: format!("{:?}", err),
//         }
//     }
// }
// impl<O, T: std::fmt::Debug> From<Result<O, T>> for Result<O, SomeError> {
//     fn from(res: Result<O, T>) -> SomeError {
//         res.map_err(|err| SomeError {
//             desc: format!("{:?}", err),
//         })
//     }
// }
// impl From<&dyn std::fmt::Debug> for SomeError {
//     fn from(err: &dyn std::fmt::Debug) -> SomeError {
//         SomeError {
//             desc: format!("{:?}", err),
//         }
//     }
// }

pub trait ToSomeError<T> {
    fn some_error(self, msg: &str) -> Result<T, SomeError>;
}

impl<T, E: Debug> ToSomeError<T> for Result<T, E> {
    fn some_error(self, msg: &str) -> Result<T, SomeError> {
        self.map_err(|err| SomeError::new(err, msg))
    }
}

// use std::error::Error;

// use serde::de::IgnoredAny;
// use serde::de::Deserializer;
// #[allow(unused_imports)]
// use serde::de::{self, Visitor, MapAccess, SeqAccess};
// use std::fmt;
// use std::collections::BTreeMap;
// use discord_lib::serde_json::Value;

// struct LiveList {
//     live: Vec<Live>,
// }

// fn live_list<'de, D>(deserializer: D) -> Result<Vec<Live>, D::Error>
// where
//     D: Deserializer<'de>,
// {
//     // struct RawLiveVisitor;
    
//     // impl<'de> Visitor<'de> for RawLiveVisitor {
//     //     type Value = RawLive;
        
//     //     fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
//     //         write!(formatter, "a string representing a 64-bit int")
//     //     }
        
//     //     fn visit_map<A>(self, map: A) -> Result<Self::Value, A::Error>
//     //         where A: MapAccess<'de>
//     //     {
//     //         // let num = v.parse().map_err(|_| {
//     //         //     E::invalid_value(de::Unexpected::Str(v), &"")
//     //         // })?;
            
//     //         // Ok(Snowflake(num))
//     //         // Ok(num)
//     //         panic!()
//     //     }
//     // }
    
//     // #[derive(Debug)]
//     // enum RawLive {
//     //     YoutubeLive,
//     //     UnknownLive,
//     // }
    
//     // impl<'de> Deserialize<'de> for RawLive {
//     //     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> 
//     //         where D: Deserializer<'de>
//     //     {
//     //         deserializer.deserialize_map(RawLiveVisitor)
//     //     }
//     // }
    
//     struct LiveListVisitor {
        
//     }
    
//     impl<'de> Visitor<'de> for LiveListVisitor {
//         type Value = Vec<Live>;
        
//         fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
//             formatter.write_str("LiveList")
//         }
        
//         fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
//             where
//                 A: SeqAccess<'de>
//         {
//             let mut values = Vec::new();
            
//             // fn get_youtube_live_fields(map: BTreeMap<&str, &str>) -> Option<Live> {
                
//             // }
            
//             // fn get_field() -> Result<>
            
//             loop {
//                 match seq.next_element::<BTreeMap<String, Value>>()? {
//                     Some(raw_live) => {
//                         if raw_live.get("live_start").map(|x| x.is_null()).unwrap_or(true) {
//                             continue
//                         }
                        
//                         match raw_live.get("yt_video_key").and_then(|x| x.as_str()) {
//                             Some(yt_video_key) => {
//                                 let live = Live {
//                                     id: yt_video_key.to_string(),
//                                     title: raw_live.get("title")
//                                         .ok_or(de::Error::missing_field("title"))?
//                                         .as_str().ok_or(de::Error::invalid_value(de::Unexpected::Other("non string title"), &"string"))?
//                                         .to_string(),
//                                     channel: raw_live.get("channel")
//                                         .and_then(|channel| {
//                                             channel.as_object()?.get("yt_channel_id")?.as_str()
//                                         })
//                                         .ok_or(de::Error::missing_field("channel.id"))?
//                                         // .ok_or(de::Error::missing_field("channel"))?
//                                         // .as_object().ok_or(de::Error::invalid_value(de::Unexpected::Other("non string"), &"string"))?
//                                         // .get("id").and_then(|channel| {
//                                         //     channel.get("yt_channel_id")
//                                         // })
//                                         // .as_str().ok_or(de::Error::invalid_value(de::Unexpected::Other("non string"), &"string"))?
//                                         .to_string(),
//                                     start_time: raw_live.get("live_start")
//                                         .ok_or(de::Error::missing_field("live_start"))?
//                                         .as_str().ok_or(de::Error::invalid_value(de::Unexpected::Other("non string live_start"), &"string"))?
//                                         .to_string(),
//                                     // start_time: raw_live.get("start")
//                                     //     .ok_or(de::Error::missing_field("startTime"))?
//                                     //     .as_u64().ok_or(de::Error::invalid_value(de::Unexpected::Other("non u64"), &"u64 timestamp"))?
//                                     //     .to_string(),
//                                 };
//                                 values.push(live);
//                             }
//                             None => ()
//                         }
//                         // match raw_live.get("platform") {
//                         //     Some(platform) => {
//                         //         if platform == &"youtube" {
//                         //             let live = Live {
//                         //                 id: raw_live.get("id")
//                         //                     .ok_or(de::Error::missing_field("id"))?
//                         //                     .as_str().ok_or(de::Error::invalid_value(de::Unexpected::Other("non string"), &"string"))?
//                         //                     .to_string(),
//                         //                 title: raw_live.get("title")
//                         //                     .ok_or(de::Error::missing_field("title"))?
//                         //                     .as_str().ok_or(de::Error::invalid_value(de::Unexpected::Other("non string"), &"string"))?
//                         //                     .to_string(),
//                         //                 channel: raw_live.get("channel")
//                         //                     .ok_or(de::Error::missing_field("channel"))?
//                         //                     .as_str().ok_or(de::Error::invalid_value(de::Unexpected::Other("non string"), &"string"))?
//                         //                     .to_string(),
//                         //                 start_time: raw_live.get("start")
//                         //                     .ok_or(de::Error::missing_field("startTime"))?
//                         //                     .as_u64().ok_or(de::Error::invalid_value(de::Unexpected::Other("non u64"), &"u64 timestamp"))?
//                         //                     .to_string(),
//                         //             };
//                         //             values.push(live);
//                         //         }
//                         //     }
//                         //     None => ()
//                         // }
//                     }
//                     None => break
//                 }
//             }
            
//             Ok(values)
//         }
//     }
    
    
//     // let mut values = Vec::new();
    
//     // loop {
//     //     match seq.next_element::<Bar>() {
//     //         Ok(Some(x)) => values.push(x),
//     //         Ok(None) => break,
//     //         Err(e) => {
//     //             if !e.to_string().starts_with("missing field") {
//     //                 return Err(e);
//     //             }
//     //         }
//     //     }
//     // }
    
//     // Ok(VecOpt(values))
//     deserializer.deserialize_any(LiveListVisitor {})
// }

// #[derive(Debug, Clone, Deserialize)]
// struct Live {
//     id: String,
//     title: String,
//     // r#type: String,
//     channel: String,
//     #[serde(default, alias = "startTime")]
//     start_time: String,
// }

// #[derive(Debug, Deserialize)]
// struct JetriLive {
//     #[serde(deserialize_with = "live_list")]
//     live: Vec<Live>,
//     #[allow(dead_code)]
//     upcoming: IgnoredAny,
// }

pub struct GenericLive {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub start_time: DateTimeF,
}

pub async fn process_generic(
    lives: &[GenericLive],
    active: &mut HashSet<String>,
    send_handle: &SendHandle,
    pool: &sqlx::PgPool,
    d_state: DState,
) -> Result<(), Error> {
    let mut out_messages: Vec<(Snowflake, String)> = Vec::new();
    // let mut channel_cache: BTreeMap<u64, Channel> = BTreeMap::new();
    
    for live in lives.iter() {
        if active.contains(&live.id) {
            continue
        }
        let ref start_time = live.start_time;
        let ref stream_name = format!("https://www.youtube.com/watch?v={}", live.id);
        
        let mut transaction = pool.begin().await.context("transaction begin")?;
        
        let subscribed_channels: Vec<(i64,)> = sqlx::query_as(r#"
            SELECT channel FROM config.subscriptions
            WHERE
                sub_id = $1 AND
                "type" = 'youtube'
        "#)
            .bind(&live.channel)
            .fetch_all(&mut *transaction)
            .await.context("get subscribed channels")?;
        
        let mut updated_channels: Vec<u64> = Vec::new();
        
        for (channel_id,) in subscribed_channels {
            let channel_id = crate::from_i(channel_id);
            
            let channel = {
                let get_res = {
                    let mut state = d_state.lock().await;
                    let channel_cache = &mut state.channel_cache;
                    channel_cache.get(&channel_id).map(|x| x.clone())
                };
                
                let channel = match get_res {
                    Some(channel) => channel,
                    None => {
                        let channel_sf = Snowflake(channel_id);
                        let channel = match send_handle.get_channel(channel_sf).await {
                                // .map_err(|e| {
                                //     println!("channel failed {}", channel_id);
                                //     e
                                // })
                                // .some_error("get channel") {
                            Ok(channel) => channel,
                            Err(err) => {
                                println!("channel failed {} {:?}", channel_id, err);
                                continue
                            }
                        };
                        // println!("{:#?}", channel);
                        {
                            let mut state = d_state.lock().await;
                            let channel_cache = &mut state.channel_cache;
                            let channel = channel_cache.entry(channel_id).or_insert(channel);
                            channel.clone()
                        }
                    }
                };
                // channel.clone()
                channel
            };
            // let channel_sf = Snowflake(crate::from_i(channel_id));
            // let channel = send_handle.get_channel(channel_sf).await.some_error("get channel")?;
            // println!("{:#?}", channel);
            
            let server_id = match channel.guild_id {
                Some(ref x) => to_i(x.0),
                None => continue,
            };
            
            {
                let d_state = &mut *d_state.lock().await;
                if let Some(ref guild_id) = channel.guild_id {
                    let in_server = d_state.servers.contains(guild_id);
                    if !in_server {
                        continue
                    }
                }
            }
            
            let (stream_id,): (i32,) = sqlx::query_as(r#"
                INSERT INTO tags.streams (name, has_server, server, readable, start_time)
                    VALUES ($1, true, $4, $2, $3)
                    ON CONFLICT ("name", "has_server", "server")
                        DO UPDATE SET has_server = tags.streams.has_server
                    RETURNING id
            "#)
                .bind(&stream_name)
                .bind(&live.title)
                // has server = ?
                // https://discord.com/developers/docs/resources/channel#get-channel
                .bind(start_time)
                .bind(server_id)
                .fetch_one(&mut *transaction).await.context("insert stream")?;
            
            sqlx::query(r#"
                INSERT INTO config.selected_streams (channel, stream)
                    VALUES ($1, $2)
                    ON CONFLICT ("channel")
                        DO UPDATE SET stream = $2
            "#)
                .bind(to_i(channel_id))
                .bind(stream_id)
                .execute(&mut *transaction)
                .await.context("set selected stream")?;
            
            updated_channels.push(channel_id)
        }
        
        for channel in updated_channels.iter() {
            let msg = format!("Active stream set <{}>", stream_name);
            // let channel = crate::from_i(*channel_i32);
            out_messages.push((Snowflake(*channel), msg));
        }
        
        transaction.commit().await.context("transaction commit")?;
    }
    *active = lives.iter().map(|x| x.id.clone()).collect();
    
    for (to, msg) in out_messages {
        // send_handle.send(to, &msg.into()).await.some_error("send message")?;
        if let Err(err) = send_handle.send(to, &msg.into()).await {
            dbg!(err);
        }
        // send_handle.send(to, &msg.into()).await.unwrap();
    }
    
    Ok(())
}

// #[allow(dead_code)]
// pub async fn auto_live_task(send_handle: &SendHandle, active: &mut HashSet<String>, pool: &sqlx::PgPool, d_state: DState) -> Result<(), SomeError> {
    
//     use discord_lib::hyper::Uri;
//     use discord_lib::{ hyper, bytes };
//     // use bytes::BufExt as _;
//     // use discord_lib::hyper::body::Buf as _;
//     use discord_lib::hyper::body::Bytes;
//     #[allow(unused_imports)]
//     use bytes::buf::BufExt as _;
    
//     // use bytes::{Buf, IntoBuf, Bytes};
//     use discord_lib::tokio::time::delay_for;
//     let client = discord_lib::send_message::get_client().some_error("get_client")?;
    
//     // let mut active: HashSet<String> = std::default::Default::default();
    
//     // let mut channel_cache: BTreeMap<u64, Channel> = BTreeMap::new();
    
//     // let res = client.get(Uri::from_static("https://api.jetri.co/live")).await.some_error("client get")?;
//     // let res = client.get(Uri::from_static("https://api.jetri.co/live/1.1")).await.some_error("client get")?;
//     let res = timeout(
//         ::std::time::Duration::new(60, 0),
//         client.get(Uri::from_static("https://api.holotools.app/v1/live"))
//     ).await.some_error("client get timeout")?.some_error("client get")?;
    
//     let body: Bytes = match timeout(::std::time::Duration::new(60, 0), hyper::body::to_bytes(res)).await {
//         Ok(x) => x,
//         Err(err) => {
//             dbg!("auto live timeout", &err);
//             delay_for(::std::time::Duration::new(60, 0)).await;
//             // continue;
//             return Err(err).some_error("timeout");
//         }
//     }.some_error("read body")?;
    
//     // let mut buf: Vec<u8> = Vec::new();
//     // buf.resize(body.remaining(), 0);
//     // body.copy_to_slice(&mut buf);
//     // let body: &[u8] = &buf;
//     let body: &[u8] = &body;
    
//     // let body_str: &str = std::str::from_utf8(body).some_error("utf8 decode")?;
//     // // println!("{:?}", &body.bytes()[539..540]);
//     // println!("{:?}", body_str);
    
//     let body_str: &str = std::str::from_utf8(body).some_error("utf8 decode")?;
//     // println!("{:?}", body_str);
    
//     // let data: JetriLive = serde_json::from_reader(body.reader()).some_error("parse json").map_err(|err| {
//     // let data: JetriLive = serde_json::from_str(body_str)
//     // .map_err(|err| {
//     //     // dbg!(err.line());
//     //     println!("{}", err);
        
//     //     if let Some(line) = body_str.lines().skip(err.line()-1).next() {
//     //         let col = err.column();
//     //         let start = ((col as isize) - 5).max(0) as usize;
//     //         let end = (col + 500).min(line.len());
            
//     //         fn find_char_boundary(s: &str, i: usize) -> usize {
//     //             let mut bound = i;
//     //             while !s.is_char_boundary(bound) {
//     //                 bound -= 1;
//     //             }
//     //             bound
//     //         }
//     //         let start = find_char_boundary(line, start);
//     //         let end = find_char_boundary(line, end);
            
//     //         let sub_str = &line[start..end];
//     //         let arrow = "     ^";
//     //         // dbg!(sub_str);
//     //         println!("{}", sub_str);
//     //         println!("{}", arrow);
//     //     } else {
//     //         println!("Invalid line number.");
//     //     }
        
//     //     err
//     // })
//     let data: JetriLive = crate::util::parse_json_print_err(body_str)
//     .some_error("parse json").map_err(|err| {
//         // dbg!(&err, body_str);
//         // dbg!(&err);
        
//         err
//     })?;
    
//     let mut generic = Vec::new();
//     for live in data.live.iter() {
//         let start_time = DateTime::parse_from_rfc3339(&live.start_time).some_error("parse_from_rfc3339")?;
//         let live = GenericLive {
//             id: live.id.clone(),
//             channel: live.channel.clone(),
//             title: live.title.clone(),
//             start_time: start_time,
//         };
//         generic.push(live);
//     }
    
//     process_generic(
//         &generic,
//         active,
//         send_handle,
//         pool,
//         d_state,
//     ).await.some_error("generic")?;
    
//     Ok(())
// }
