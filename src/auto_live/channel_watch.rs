
use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::{Result};
use async_trait::async_trait;
use linkify::{LinkFinder, LinkKind};

use ::tokio::sync::RwLock;

use crate::i_love_youtube::extract_id as yt_extract_id;
use crate::i_love_youtube::get_stream_info;
use crate::twitch::extract_id as twitch_extract_id;
use crate::{from_i, to_i};

use crate::handlers::{ Handler, HandlerResult, HandlerError, Command };
use discord_lib::discord::Snowflake;

use sqlx::PgPool;

#[derive(Debug, PartialEq)]
enum FoundUrl {
    Youtube{video_id: String},
    Twitch{video_id: String},
    None,
}

fn find_yt_url(input: &str) -> FoundUrl {
    let mut finder = LinkFinder::new();
    finder.kinds(&[LinkKind::Url]);
    
    let links: Vec<_> = finder.links(input).collect();
    
    for link in links {
        let link_str = link.as_str();
        // dbg!(link_str);
        if let Some(id) = yt_extract_id(link_str) {
            return FoundUrl::Youtube{ video_id: id.into() }
        }
        if let Some(id) = twitch_extract_id(link_str) {
            // dbg!(&id);
            return FoundUrl::Twitch{ video_id: id.into() }
        }
    }
    
    FoundUrl::None
}

// watched discord channel id -> target/tagging discord channel id
type SubMap = BTreeMap<u64, Vec<u64>>;
// wrapped sub map
type WSubMap = Arc<RwLock<SubMap>>;

#[derive(Debug)]
pub struct ChannelWatchHandler {
    sub_map: WSubMap,
}

impl ChannelWatchHandler {
    pub fn new(pool: PgPool) -> Self {
        let sub_map = Arc::new(RwLock::new(SubMap::new()));
        let task_sub_map = sub_map.clone();
        
        let task_pool = pool.clone();
        
        ::tokio::task::spawn(async move {
            let mut sub_map = task_sub_map;
            let pool = task_pool;
            
            loop {
                if let Err(err) = Self::sync_sub_map(&pool, &mut sub_map).await {
                    println!("channel watch sub list update failed");
                    dbg!(err);
                }
                
                // delay_for(::std::time::Duration::new(300, 0)).await;
                ::tokio::time::sleep(::std::time::Duration::new(300, 0)).await;
            }
        });
        
        ChannelWatchHandler {
            sub_map,
            // pool,
            // d_state,
        }
    }
    
    async fn sync_sub_map(pool: &PgPool, sub_map: &WSubMap) -> Result<()> {
        let subs: Vec<(i64, String)> = sqlx::query_as(r#"
            SELECT channel, sub_id
            FROM config.subscriptions
            WHERE type = 'watch_channel'
        "#)
            // .bind(stream.id)
            .fetch_all(pool).await.map_err(|e| {
                // eprintln!("get offsets {:?}", e);
                // HandlerError::with_message("DB error".into())
                e
            })?;
        
        let mut sub_map = sub_map.write().await;
        sub_map.clear();
        for (target_channel, watch_channel) in subs {
            let target_channel = from_i(target_channel);
            let watch_channel: u64 = match watch_channel.parse() {
                Ok(x) => x,
                Err(_err) => {
                    continue
                }
            };
            let targets = sub_map.entry(watch_channel)
                .or_insert_with(|| Vec::new());
            targets.push(target_channel);
            // sub_map.insert(watch_channel, target_channel);
        }
        std::mem::drop(sub_map);
        
        Ok(())
    }
}

#[async_trait]
impl Handler for ChannelWatchHandler {
    async fn handle_message_b(&self, cmd: Command<'_>) -> HandlerResult {
        let msg = cmd.message;
        let pool = cmd.pool;
        
        let sub_map = self.sub_map.read().await;
        let targets = match sub_map.get(&msg.channel_id.0) {
            Some(targets) => {
                if targets.is_empty() {
                    return Ok(().into())
                }
                targets.clone()
            }
            None => return Ok(().into()),
        };
        std::mem::drop(sub_map);
        
        let stream_name: String;
        let readable: Option<String>;
        let start_time: chrono::DateTime<chrono::FixedOffset>;
        let mut got_info: bool = false;
        // let mut start_time;
        // if let Some(video_id) = find_yt_url(&msg.content) {
        match find_yt_url(&msg.content) {
            FoundUrl::Youtube{ video_id } => {
                // let ref stream_name = format!("https://www.youtube.com/watch?v={}", video_id);
                stream_name = format!("https://www.youtube.com/watch?v={}", video_id);
                
                let stream_info = match get_stream_info(&video_id).await {
                    Ok(x) => {
                        got_info = true;
                        Some(x)
                    }
                    Err(err) => {
                        eprintln!("cwatch get video info failed {}", &video_id);
                        dbg!(err);
                        None
                    }
                };
                
                start_time = stream_info.as_ref()
                    .map(|x| x.best_start_time)
                    .unwrap_or_else(|| chrono::Utc::now().into());
                
                readable = stream_info.map(|x| x.title);
            }
            FoundUrl::Twitch{ video_id } => {
                stream_name = format!("https://www.twitch.tv/videos/{}", video_id);
                
                start_time = match crate::twitch::get_start_time(&video_id).await {
                    Ok(x) => {
                        got_info = true;
                        x
                    }
                    Err(err) => {
                        eprintln!("cwatch get twitch video info failed {}", &video_id);
                        dbg!(err);
                        // None
                        chrono::Utc::now().into()
                    }
                };
                
                readable = None;
            }
            _ => {
                return Ok(().into())
            }
        }
        
        // println!("{} {:?}", stream_name, targets);
        
        for target_channel in targets {
            let channel = {
                cmd.state.get_channel(Snowflake(target_channel)).await
            };
            let channel = match channel {
                Ok(c) => c,
                Err(err) => {
                    eprintln!("cwatch get channel failed");
                    dbg!(err);
                    continue
                }
            };
            
            let server_id = match channel.guild_id.map(|s| s.0) {
                Some(x) => x,
                None => {
                    eprintln!("cwatch not in guild target: {}", target_channel);
                    continue
                }
            };
            
            let res = sqlx::query_as(r#"
                INSERT INTO tags.streams (name, has_server, server, readable, start_time)
                    VALUES ($1, true, $4, $2, $3)
                    ON CONFLICT ("name", "has_server", "server")
                        DO UPDATE SET has_server = tags.streams.has_server
                    RETURNING id
            "#)
                // .bind(&stream_name)
                .bind(&stream_name)
                // .bind(&stream_info.as_ref().map(|x| &x.title))
                .bind(readable.as_ref())
                // .bind(&live.title)
                // // has server = ?
                // // https://discord.com/developers/docs/resources/channel#get-channel
                // .bind(stream_info.as_ref()
                //     .map(|x| x.best_start_time)
                //     .unwrap_or_else(|| chrono::Utc::now().into()))
                .bind(start_time)
                .bind(to_i(server_id))
                // .bind(start_time)
                // .bind(server_id)
                .fetch_one(pool).await;
                // .map_err(|e| {
                //     eprintln!("channel watch insert stream {:?}", e);
                //     HandlerError::with_message("DB error".into())
                // })?;
            
            let (stream_id,): (i32,) = match res {
                Ok(x) => x,
                Err(err) => {
                    eprintln!("channel watch insert stream {:?}", err);
                    continue
                }
            };
            
            let res = sqlx::query(r#"
                INSERT INTO config.selected_streams (channel, stream)
                    VALUES ($1, $2)
                    ON CONFLICT ("channel")
                        DO UPDATE SET stream = $2
            "#)
                .bind(to_i(target_channel))
                .bind(stream_id)
                .execute(pool)
                .await;
            
            match res {
                Ok(_) => (),
                Err(err) => {
                    eprintln!("channel watch set selected stream {:?}", err);
                    continue
                }
            };
            
            // let msg = format!("Active stream set{} <{}>", stream_info.map_or("*", |_| ""), stream_name);
            let msg = format!("Active stream set{} <{}>", if got_info {"*"} else {""}, stream_name);
            if let Err(err) = cmd.state.send_handle.send(Snowflake(target_channel), &msg.into()).await {
                dbg!(err);
            }
        }
        
        // Ok("_".into())
        Ok(().into())
    }
    
    async fn handle_command_b(&self, command: Command<'_>) -> HandlerResult {
        command.require_admin().await?;
        
        let op: &str = command.args.get(0)
            .map(|x| *x)
            .ok_or(HandlerError::with_message("Missing param".into()))?;
        
        match op {
            "sync" => {
                Self::sync_sub_map(command.pool, &self.sub_map).await
                    .map_err(|err| {
                        dbg!(err);
                        HandlerError::with_message("Failed".into())
                    })?;
                Ok("_".into())
            }
            _ => {
                Err(HandlerError::with_message("Invalid op".into()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ find_yt_url, FoundUrl };
    
    #[test]
    fn non_url() {
        assert_eq!(find_yt_url("a"), FoundUrl::None);
    }
    
    #[test]
    fn has_youtube_url() {
        assert_eq!(find_yt_url("http://www.youtube.com/watch?v=aaaaaa"), FoundUrl::Youtube{ video_id: "aaaaaa".into() });
        assert_eq!(find_yt_url("uxbeix http://www.youtube.com/watch?v=aaaaaa aoeui"), FoundUrl::Youtube{ video_id: "aaaaaa".into() });
    }
    
    #[test]
    fn has_twitch_url() {
        assert_eq!(find_yt_url("https://www.twitch.tv/videos/aaaaaa"), FoundUrl::Twitch{ video_id: "aaaaaa".into() });
        assert_eq!(find_yt_url("uxbeix https://www.twitch.tv/videos/aaaaaa aoeui"), FoundUrl::Twitch{ video_id: "aaaaaa".into() });
    }
    
    #[test]
    fn wrong_host() {
        assert_eq!(find_yt_url("http://www.twitch.com/watch?v=aaaaaa"), FoundUrl::None);
        assert_eq!(find_yt_url("uxbeix http://www.twitch.com/watch?v=aaaaaa aoeui"), FoundUrl::None);
    }
    
    #[test]
    fn has_email() {
        assert_eq!(find_yt_url("aaaabbb@gmail.com"), FoundUrl::None);
        assert_eq!(find_yt_url("uxbeix aaaabbb@gmail.com aoeui"), FoundUrl::None);
    }
}
