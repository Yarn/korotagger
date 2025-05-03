
use std::fmt::Debug;
use std::collections::HashSet;
use std::collections::HashMap;
use std::time::Instant;
use discord_lib::SendHandle;
use discord_lib::discord::Snowflake;
#[allow(unused_imports)]
use chrono::Utc;
#[allow(unused_imports)]
use chrono::DateTime;
#[allow(unused_imports)]
use crate::i_love_youtube::get_stream_start_time;
#[allow(unused_imports)]
use crate::{from_i, to_i};
use crate::DateTimeF;
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

pub trait ToSomeError<T> {
    fn some_error(self, msg: &str) -> Result<T, SomeError>;
}

impl<T, E: Debug> ToSomeError<T> for Result<T, E> {
    fn some_error(self, msg: &str) -> Result<T, SomeError> {
        self.map_err(|err| SomeError::new(err, msg))
    }
}

use crate::DiscordState;

pub type ChannelCache = HashMap<u64, (Option<u64>, Instant)>;

async fn get_channel_guild(
    channel_cache: &mut HashMap<u64, (Option<u64>, Instant)>,
    states: &[DiscordState],
    channel_id: Snowflake,
) -> Option<Snowflake> {
    if let Some((guild, last_fetch)) = channel_cache.get(&channel_id.0) {
        // refetch after one day
        if last_fetch.elapsed().as_secs() < 60*60*24 {
            return guild.map(|id| Snowflake(id)).clone();
        }
    }
    
    // let mut from_cache = None;
    for state in states {
        
        let guild_id = match state.send_handle.get_channel(channel_id).await {
            Ok(channel) => channel.guild_id,
            Err(err) => {
                println!("channel failed {:?} {:?}", channel_id, err);
                return None
            }
        };
        
        if let Some(guild_id) = guild_id {
            channel_cache.insert(channel_id.0, (Some(guild_id.0), Instant::now()));
            return Some(guild_id)
        }
    }
    
    channel_cache.insert(channel_id.0, (None, Instant::now()));
    
    None
}

pub struct GenericLive {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub start_time: DateTimeF,
}

pub async fn process_generic(
    lives: &[GenericLive],
    active: &mut HashSet<String>,
    pool: &sqlx::PgPool,
    states: &[crate::DiscordState],
    channel_cache: &mut ChannelCache,
) -> Result<(), Error> {
    let mut out_messages: Vec<(Snowflake, &SendHandle, String)> = Vec::new();
    
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
        
        let mut updated_channels: Vec<(u64, &SendHandle)> = Vec::new();
        
        for (channel_id,) in subscribed_channels {
            let channel_id = crate::from_i(channel_id);
            
            let guild_id = get_channel_guild(
                channel_cache,
                states,
                Snowflake(channel_id),
            ).await;
            let guild_id = if let Some(guild_id) = guild_id {
                guild_id
            } else {
                continue
            };
            
            let mut in_any_server = false;
            for state in states {
                let mut servers = state.servers.lock().await;
                let servers = &mut *servers;
                let in_server = servers.contains(&guild_id);
                if in_server {
                    updated_channels.push((channel_id, &state.send_handle));
                    in_any_server = true
                }
            }
            if !in_any_server {
                continue
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
                .bind(to_i(guild_id.0))
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
        }
        
        for (channel, send_handle) in updated_channels.iter() {
            let msg = format!("Active stream set <{}>", stream_name);
            out_messages.push((Snowflake(*channel), send_handle, msg));
        }
        
        transaction.commit().await.context("transaction commit")?;
    }
    *active = lives.iter().map(|x| x.id.clone()).collect();
    
    for (to, send_handle, msg) in out_messages {
        if let Err(err) = send_handle.send(to, &msg.into()).await {
            dbg!(err);
        }
    }
    
    Ok(())
}
