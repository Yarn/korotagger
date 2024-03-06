
use async_trait::async_trait;

use super::{ Handler, HandlerResult, HandlerError, Command };
use crate::i_love_youtube::extract_id;
use crate::i_love_youtube;
use crate::twitch;
use crate::auto_live::twitter_spaces;

// use crate::DB;
use crate::{
    Offset,
    Stream,
};
use sqlx::PgPool;
use crate::to_i;
// use crate::DateTimeF;
use chrono::Utc;
use time::Duration;

#[derive(Debug)]
pub struct SetStreamHandler {
    pub pool: PgPool,
}

#[async_trait]
impl Handler for SetStreamHandler {
    async fn handle_command_b(&self, command: Command<'_>) -> HandlerResult {
        // command.require_admin().await?;
        
        let stream_name = command.args.get(0).ok_or(HandlerError::with_message("Param required".into()))?;
        let stream_name = stream_name.trim_matches('<').trim_matches('>');
        
        let ref stream_name = match extract_id(&stream_name) {
            Some(yt_id) => {
                format!("https://www.youtube.com/watch?v={}", yt_id)
            }
            None => stream_name.to_string()
        };
        
        let stream = Stream {
            tags: Vec::new(),
            offsets: vec![Offset { position: 0, offset: -20 }],
            start_time: Utc::now().into(),
        };
        
        let ref server_id = command.message.guild_id.as_ref()
            .ok_or(HandlerError::with_message("Not in a server".into()))?;
        
        let mut transaction = self.pool.begin().await.map_err(|e| {
            eprintln!("transaction begin {:?}", e);
            HandlerError::with_message("DB error".into())
        })?;
        
        let stream_id: Option<(i32,)> = sqlx::query_as(r#"
            SELECT "id"
            FROM tags.streams
            WHERE "name" = $1 and server = $2
            ORDER BY has_server DESC
        "#)
            .bind(&stream_name)
            .bind(to_i(server_id.0))
            .fetch_optional(&mut transaction).await
            .map_err(|e| {
                eprintln!("find stream {:?}", e);
                HandlerError::with_message("DB error".into())
            })?;
        
        let stream_id = match stream_id {
            Some(stream_id) => stream_id.0,
            None => {
                let (stream_id,): (i32,) = sqlx::query_as(r#"
                    INSERT INTO tags.streams (name, has_server, server, readable, start_time)
                        VALUES ($1, true, $2, NULL, $3)
                        --ON CONFLICT ("name", "has_server", "server")
                        --    DO UPDATE SET has_server = tags.streams.has_server
                        RETURNING id
                "#)
                    .bind(&stream_name)
                    // .bind(server_id.is_some())
                    // .bind(server_id.as_ref().map(|x| to_i(x.0)))
                    .bind(to_i(server_id.0))
                    // .bind(None::<String>)
                    // .bind(&live.title)
                    .bind(stream.start_time)
                    .fetch_one(&mut transaction).await
                    .map_err(|e| {
                        eprintln!("create stream {:?}", e);
                        HandlerError::with_message("DB error".into())
                    })?;
                stream_id
            }
        };
        
        
        
        let channel_id = command.message.channel_id.0;
        
        sqlx::query(r#"
            INSERT INTO config.selected_streams (channel, stream)
                VALUES ($1, $2)
                ON CONFLICT ("channel")
                    DO UPDATE SET stream = $2
        "#)
            .bind(to_i(channel_id))
            .bind(stream_id)
            .execute(&mut transaction).await
            .map_err(|e| {
                eprintln!("set selected stream {:?}", e);
                HandlerError::with_message("DB error".into())
            })?;
        
        transaction.commit().await.map_err(|e| {
            eprintln!("transaction commit {:?}", e);
            HandlerError::with_message("DB error".into())
        })?;
        
        // {
        //     let mut data = DB.borrow_data_mut().unwrap();
        //     let channel_id = command.message.channel_id.0;
        //     data.current_stream.insert(channel_id, stream_name.into());
        //     data.streams.entry(stream_name.into()).or_insert(stream);
        // }
        // DB.async_save_data().await.unwrap();
        
        Ok("_".into())
    }
}

#[derive(Debug)]
pub struct ListStreamsHandler {
    pub pool: PgPool,
}

#[async_trait]
impl Handler for ListStreamsHandler {
    async fn handle_command_b(&self, _command: Command<'_>) -> HandlerResult {
        let mut out = String::new();
        out.push_str("_\n");
        // {
        //     let data = DB.borrow_data().unwrap();
        //     // data.insert(stream_name.into(), 0);
        //     for (k, v) in data.streams.iter() {
        //         if v.tags.len() >= 1 {
        //             out.push_str(&format!("<{}>, {}, {} tags\n", k, v.start_time, v.tags.len()));
        //         }
        //     }
        // }
        
        let streams: Vec<(String, chrono::NaiveDateTime, i64)> = sqlx::query_as(r#"
            SELECT streams.name, start_time, COUNT(tags.id) FROM tags.streams, tags.tags
            WHERE stream = streams.id
            GROUP BY streams.id;
        "#)
            .fetch_all(&self.pool)
            .await.map_err(|e| {
                eprintln!("get streams {:?}", e);
                HandlerError::with_message("DB error".into())
            })?;
        
        for (stream_name, start_time, tag_count) in streams.iter() {
            out.push_str(&format!("<{}>, {}, {} tags\n", stream_name, start_time, tag_count));
        }
        
        Ok(out.into())
    }
}

#[derive(Debug)]
pub struct YtStartHandler {
    pub pool: PgPool,
}

#[async_trait]
impl Handler for YtStartHandler {
    async fn handle_command_b(&self, command: Command<'_>) -> HandlerResult {
        // command.require_admin().await?;
        
        let channel_id = command.message.channel_id.0;
        
        // let current_stream: String = {
        //     let data = DB.borrow_data().unwrap();
        //     let channel_id = command.message.channel_id.0;
        //     match data.current_stream.get(&channel_id) {
        //         Some(s) => s.to_string(),
        //         None => {
        //             std::mem::drop(data);
        //             return Ok("No active stream".into())
        //         }
        //     }
        // };
        
        let mut transaction = self.pool.begin().await.map_err(|e| {
            eprintln!("transaction begin {:?}", e);
            HandlerError::with_message("DB error".into())
        })?;
        
        // let selected_stream: Option<(String, i32)> = sqlx::query_as(r#"
        //     SELECT streams.name, streams.id, * FROM config.selected_streams, tags.streams
        //     WHERE channel = $1 and stream = streams.id;
        // "#)
        //     .bind(to_i(channel_id))
        //     .fetch_optional(&mut transaction).await
        //     .map_err(|e| {
        //         eprintln!("update start time {:?}", e);
        //         HandlerError::with_message("DB error".into())
        //     })?;
        
        // let (current_stream, pg_stream_id) = match selected_stream {
        //     Some(x) => x,
        //     None => {
        //         return Ok("No active stream".into())
        //     }
        // };
        
        let stream = crate::db_util::get_current_stream(&mut transaction, channel_id)
            .await.map_err(|e| {
                eprintln!("update start time {:?}", e);
                HandlerError::with_message("DB error".into())
            })?;
        
        let (current_stream, pg_stream_id) = match stream {
            Some(stream) => (stream.name, stream.id),
            None => {
                return Ok("No active stream".into())
            }
        };
        
        let stream_id = match extract_id(&current_stream) {
            Some(x) => x,
            None => {
                return Ok("Could not find youtube id from url".into())
            }
        };
        
        let stream_start_time = match i_love_youtube::get_stream_start_time(stream_id).await {
            Ok(start_time) => start_time,
            Err(err) => {
                dbg!(err);
                return Ok("Could not get start time from youtube".into())
            }
        };
        
        
        sqlx::query(r#"
            UPDATE tags.streams
            SET start_time = $1
            WHERE "id" = $2
        "#)
            .bind(stream_start_time)
            .bind(pg_stream_id)
            .execute(&mut transaction).await
            .map_err(|e| {
                eprintln!("update start time {:?}", e);
                HandlerError::with_message("DB error".into())
            })?;
        // sqlx::query(r#"
        //     UPDATE tags.streams
        //     SET start_time = $1
        //     WHERE "id" IN (
        //         SELECT "stream" FROM config.selected_streams
        //         WHERE channel = $2
        //     )
        // "#)
        //     .bind(stream_start_time)
        //     .bind(to_i(channel_id))
        //     .execute(&self.pool).await
        //     .map_err(|e| {
        //         eprintln!("update start time {:?}", e);
        //         HandlerError::with_message("DB error".into())
        //     })?;
        // sqlx::query(r#"
        //     DELETE FROM tags.tag_offsets
        //     USING tags.tags, tags.streams
        //     WHERE
        //         tags.id = tag_offsets.tag AND
        //         tags.stream = streams.id AND
        //         streams.id = $1
        // "#)
        //     .bind(pg_stream_id)
        //     .execute(&mut transaction).await
        //     .map_err(|e| {
        //         eprintln!("update start time {:?}", e);
        //         HandlerError::with_message("DB error".into())
        //     })?;
        sqlx::query(r#"
            DELETE FROM tags.stream_offsets
            USING tags.streams
            WHERE
                stream_offsets.stream = streams.id AND
                streams.id = $1
        "#)
            .bind(pg_stream_id)
            .execute(&mut transaction).await
            .map_err(|e| {
                eprintln!("update start time {:?}", e);
                HandlerError::with_message("DB error".into())
            })?;
        
        transaction.commit().await.map_err(|e| {
            eprintln!("transaction commit {:?}", e);
            HandlerError::with_message("DB error".into())
        })?;
        
        // {
        //     let mut data = DB.borrow_data_mut().unwrap();
            
        //     let mut stream = data.streams.get_mut(&current_stream).unwrap();
            
        //     stream.start_time = stream_start_time;
        // }
        
        Ok("_".into())
    }
}

#[derive(Debug)]
pub struct TwitchStartHandler {
    pub pool: PgPool,
}

#[async_trait]
impl Handler for TwitchStartHandler {
    async fn handle_command_b(&self, command: Command<'_>) -> HandlerResult {
        // command.require_admin().await?;
        
        let channel_id = command.message.channel_id.0;
        
        let video_id: Option<String> = match command.args {
            [] => None,
            [name] => {
                let name = name.trim_matches('<').trim_matches('>');
                let video_id = twitch::extract_id(name)
                    .ok_or(HandlerError::with_message("can not extract video id".into()))?;
                Some(video_id)
            }
            _ => {
                return Err(HandlerError::with_message("invalid parameters".into()));
            }
        };
        
        let mut transaction = self.pool.begin().await.map_err(|e| {
            eprintln!("transaction begin {:?}", e);
            HandlerError::with_message("DB error".into())
        })?;
        
        let stream = crate::db_util::get_current_stream(&mut transaction, channel_id)
            .await.map_err(|e| {
                eprintln!("update start time {:?}", e);
                HandlerError::with_message("DB error".into())
            })?;
        
        let (current_stream, pg_stream_id) = match stream {
            Some(stream) => (stream.name, stream.id),
            None => {
                return Ok("No active stream".into())
            }
        };
        let video_id = match video_id {
            Some(id) => id,
            None => {
                let stream_id = match twitch::extract_id(&current_stream) {
                    Some(x) => x,
                    None => {
                        let msg = format!("can not extract video id from `{}`", current_stream);
                        return Err(HandlerError::with_message(msg))
                    }
                };
                stream_id
            }
        };
        
        let stream_start_time = match twitch::get_start_time(&video_id).await {
            Ok(start_time) => start_time,
            Err(err) => {
                dbg!(err);
                return Ok("Could not get start time from twitch".into())
            }
        };
        
        sqlx::query(r#"
            UPDATE tags.streams
            SET start_time = $1
            WHERE "id" = $2
        "#)
            .bind(stream_start_time)
            .bind(pg_stream_id)
            .execute(&mut transaction).await
            .map_err(|e| {
                eprintln!("update start time {:?}", e);
                HandlerError::with_message("DB error".into())
            })?;
        
        sqlx::query(r#"
            DELETE FROM tags.stream_offsets
            USING tags.streams
            WHERE
                stream_offsets.stream = streams.id AND
                streams.id = $1
        "#)
            .bind(pg_stream_id)
            .execute(&mut transaction).await
            .map_err(|e| {
                eprintln!("update start time {:?}", e);
                HandlerError::with_message("DB error".into())
            })?;
        
        transaction.commit().await.map_err(|e| {
            eprintln!("transaction commit {:?}", e);
            HandlerError::with_message("DB error".into())
        })?;
        
        Ok("_".into())
    }
}

#[derive(Debug)]
pub struct TwitterSpaceStartHandler {
    pub pool: PgPool,
}

#[async_trait]
impl Handler for TwitterSpaceStartHandler {
    async fn handle_command_b(&self, command: Command<'_>) -> HandlerResult {
        // command.require_admin().await?;
        
        let channel_id = command.message.channel_id.0;
        
        let video_id: Option<String> = match command.args {
            [] => None,
            [name] => {
                let name = name.trim_matches('<').trim_matches('>');
                let video_id = twitter_spaces::extract_id(name)
                    .ok_or(HandlerError::with_message("can not extract video id".into()))?;
                Some(video_id)
            }
            _ => {
                return Err(HandlerError::with_message("invalid parameters".into()));
            }
        };
        
        let mut transaction = self.pool.begin().await.map_err(|e| {
            eprintln!("transaction begin {:?}", e);
            HandlerError::with_message("DB error".into())
        })?;
        
        let stream = crate::db_util::get_current_stream(&mut transaction, channel_id)
            .await.map_err(|e| {
                eprintln!("update start time {:?}", e);
                HandlerError::with_message("DB error".into())
            })?;
        
        let (current_stream, pg_stream_id) = match stream {
            Some(stream) => (stream.name, stream.id),
            None => {
                return Ok("No active stream".into())
            }
        };
        let video_id = match video_id {
            Some(id) => id,
            None => {
                let stream_id = match twitter_spaces::extract_id(&current_stream) {
                    Some(x) => x,
                    None => {
                        let msg = format!("can not extract video id from `{}`", current_stream);
                        return Err(HandlerError::with_message(msg))
                    }
                };
                stream_id
            }
        };
        
        let stream_start_time = match twitter_spaces::get_start_time(&video_id).await {
            Ok(start_time) => start_time,
            Err(err) => {
                dbg!(err);
                return Ok("Could not get start time from twitter".into())
            }
        };
        
        sqlx::query(r#"
            UPDATE tags.streams
            SET start_time = $1
            WHERE "id" = $2
        "#)
            .bind(stream_start_time)
            .bind(pg_stream_id)
            .execute(&mut transaction).await
            .map_err(|e| {
                eprintln!("update start time {:?}", e);
                HandlerError::with_message("DB error".into())
            })?;
        
        sqlx::query(r#"
            DELETE FROM tags.stream_offsets
            USING tags.streams
            WHERE
                stream_offsets.stream = streams.id AND
                streams.id = $1
        "#)
            .bind(pg_stream_id)
            .execute(&mut transaction).await
            .map_err(|e| {
                eprintln!("update start time {:?}", e);
                HandlerError::with_message("DB error".into())
            })?;
        
        transaction.commit().await.map_err(|e| {
            eprintln!("transaction commit {:?}", e);
            HandlerError::with_message("DB error".into())
        })?;
        
        Ok("_".into())
    }
}

#[derive(Debug)]
pub struct OffsetHandler {
    pub pool: PgPool,
}

#[async_trait]
impl Handler for OffsetHandler {
    async fn handle_command_b(&self, command: Command<'_>) -> HandlerResult {
        // command.require_admin().await?;
        
        let args = command.args;
        
        const PARAMS_MESSAGE: &str = "Invalid params, !offset <time> <amount> [end]";
        
        let mut end: Option<i64> = None;
        if args.len() == 3 {
            end = Some(match args[2].parse() { Ok(x) => x, Err(_) => { return Err(HandlerError::with_message(PARAMS_MESSAGE.into())) } });
        } else if args.len() != 2 {
            return Err(HandlerError::with_message(PARAMS_MESSAGE.into()))
        }
        
        let offset: Offset = if let (Ok(position), Ok(offset_sec)) = (args[0].parse::<i64>(), args[1].parse::<i64>()) {
            let offset = Offset {
                position: position,
                offset: offset_sec,
            };
            offset
        } else {
            return Err(HandlerError::with_message(PARAMS_MESSAGE.into()))
        };
        
        let channel_id = command.message.channel_id.0;
        
        let mut transaction = self.pool.begin().await.map_err(|e| {
            eprintln!("transaction begin {:?}", e);
            HandlerError::with_message("DB error".into())
        })?;
        
        let stream = crate::db_util::get_current_stream(&mut transaction, channel_id)
            .await.map_err(|e| {
                eprintln!("get current stream {:?}", e);
                HandlerError::with_message("DB error".into())
            })?;
        
        let (_current_stream, pg_stream_id) = match stream {
            Some(stream) => (stream.name, stream.id),
            None => {
                return Ok("No active stream".into())
            }
        };
        
        let order_value: i32 = sqlx::query_as(r#"
            SELECT "order" FROM tags.stream_offsets
            WHERE stream = $1
            ORDER BY "order" DESC
            LIMIT 1
        "#)
            .bind(pg_stream_id)
            .fetch_optional(&mut transaction).await.map_err(|e| {
                eprintln!("get order value {:?}", e);
                HandlerError::with_message("DB error".into())
            })?
            .map(|(x,): (i32,)| x+1)
            .unwrap_or(0);
        
        sqlx::query(r#"
            INSERT INTO tags.stream_offsets ("order", stream, position, "offset", "end")
            VALUES ($1, $2, $3, $4, $5)
        "#)
            .bind(order_value)
            .bind(pg_stream_id)
            .bind(Duration::seconds(offset.position))
            .bind(Duration::seconds(offset.offset))
            .bind(end.map(|x| Duration::seconds(x)))
            .execute(&mut transaction).await.map_err(|e| {
                eprintln!("insert stream offset {:?}", e);
                HandlerError::with_message("DB error".into())
            })?;
        
        // {
        //     let mut data = DB.borrow_data_mut().unwrap();
            
        //     let channel_id = command.message.channel_id.0;
        //     if data.current_stream.get(&channel_id).is_none() {
        //         std::mem::drop(data);
        //         return Ok("No active stream".into())
        //     }
        //     let ref current_stream = data.current_stream[&channel_id].to_string();
        //     data.streams.get_mut(current_stream).unwrap().offsets.push(offset);
        // }
        
        // DB.async_save_data().await.unwrap();
        
        transaction.commit().await.map_err(|e| {
            eprintln!("transaction commit {:?}", e);
            HandlerError::with_message("DB error".into())
        })?;
        
        Ok("_".into())
    }
}
