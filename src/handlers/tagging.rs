
use async_trait::async_trait;

use super::{ Handler, HandlerResult, HandlerResponse, HandlerError, SimpleHelpInfo };
use discord_lib::discord::Message;
use discord_lib::gateway::{ MessageReactionAdd, MessageReactionRemove };
use crate::message_formatting::escape_embed;
use crate::video_id::VideoId;

use crate::{ from_i, to_i };
use crate::db_util;
use sqlx::PgPool;
use chrono::Duration;
use crate::util::seconds_f64;

type DateTime = chrono::NaiveDateTime;

fn format_time_offset(mut time_offset: Duration, hour: &str, min: &str, sec: &str) -> String {
    let mut time_offset_str = String::new();
    let offset_hours = time_offset.num_hours();
    if offset_hours != 0 {
        time_offset_str.push_str(&format!("{}", offset_hours));
        time_offset_str.push_str(hour);
        time_offset = time_offset - Duration::hours(offset_hours);
    }
    let offset_minutes = time_offset.num_minutes();
    if offset_minutes != 0 {
        time_offset_str.push_str(&format!("{}", offset_minutes));
        time_offset_str.push_str(min);
        time_offset = time_offset - Duration::minutes(offset_minutes);
    }
    time_offset_str.push_str(&format!("{}", time_offset.num_seconds()));
    time_offset_str.push_str(sec);
    
    time_offset_str
}

fn format_time_offset_yt(mut time_offset: Duration) -> String {
    let mut out = String::new();
    
    let offset_hours = time_offset.num_hours();
    if offset_hours != 0 {
        out.push_str(&format!("{}:", offset_hours));
        time_offset = time_offset - Duration::hours(offset_hours);
    }
    time_offset = time_offset.abs();
    let offset_minutes = time_offset.num_minutes();
    if offset_hours != 0 || offset_minutes != 0 {
        out.push_str(&format!("{:0>2}:", offset_minutes));
        time_offset = time_offset - Duration::minutes(offset_minutes);
    }
    out.push_str(&format!("{:0>2}", time_offset.num_seconds()));
    
    out
}

fn format_tag_standard(
    out: &mut String,
    video_id: &VideoId,
    tag_name: &str, tag_votes: i32,
    _stream_name: &str, time_offset: Duration,
) {
    let tag_name = escape_embed(tag_name);
    let time_offset_str = format_time_offset( time_offset, "h", "m", "s");
    
    let offset_string = video_id.format_discord_offset(&time_offset_str);
    
    out.push_str(tag_name.as_ref());
    out.push(' ');
    
    if tag_votes != 0 {
        out.push_str(&format!("({}) ", tag_votes));
    }
    
    out.push_str(&offset_string);
    out.push('\n');
}

fn format_tag_yt(
    out: &mut String,
    tag_name: &str, _tag_votes: i32,
    _stream_name: &str, time_offset: Duration,
) {
    let tag_name = escape_embed(tag_name);
    let time_offset_str = format_time_offset_yt(time_offset);
    
    out.push_str(&time_offset_str);
    out.push(' ');
    out.push_str(&tag_name);
    
    out.push('\n');
}

fn format_tag_csv(
    out: &mut String,
    tag_name: &str, tag_votes: i32,
    _stream_name: &str, time_offset: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let tag_name = &*escape_embed(tag_name);
    
    let cursor: std::io::Cursor<Vec<u8>> = std::io::Cursor::new(Vec::new());
    // cursor.get_mut().clear();
    let mut writer = csv::Writer::from_writer(cursor);
    
    writer.write_field(tag_name)?;
    writer.write_field(&format!("{}", tag_votes))?;
    writer.write_field(&format!("{}", time_offset.num_seconds()))?;
    
    writer.write_record(None::<&[u8]>)?;
    
    writer.flush()?;
    
    let cursor = writer.into_inner()?;
    
    let line_slice = cursor.get_ref().as_slice();
    let line_str = std::str::from_utf8(line_slice)?;
    
    out.push_str(line_str);
    
    Ok(())
}

#[derive(Debug)]
pub struct TagsHandler {
    pub pool: PgPool,
}

#[async_trait]
impl Handler for TagsHandler {
    async fn handle_command(&self, args: &[&str], msg: &Message) -> HandlerResult {
        
        let stream_name = *args.last().unwrap_or(&"_");
        let stream_name = stream_name.trim_matches('<').trim_matches('>');
        
        let flags = args.split_last().map(|x| x.1).unwrap_or(&[]);
        let own_tags = flags.contains(&"own");
        let cross_server = flags.contains(&"c");
        // only displays tags after stream start and not more than 12 hours after stream start
        let limit_time = flags.contains(&"lim");
        let info_only = flags.contains(&"info");
        
        let as_name: Option<&str> = flags.iter().position(|&s| s == "as").and_then(|as_i| args.get(as_i+1).map(|s| (*s).trim_matches('<').trim_matches('>')));
        
        let format_csv = flags.contains(&"csv");
        let format_yt = flags.contains(&"yt");
        
        let user_id: u64 = msg.author.id.0;
        let channel_id = msg.channel_id.0;
        let ref server_id = msg.guild_id.as_ref()
            .ok_or(HandlerError::with_message("Not in a server".into()))?;
        
        let mut transaction = self.pool.begin().await.map_err(|e| {
            eprintln!("transaction begin {:?}", e);
            HandlerError::with_message("DB error".into())
        })?;
        
        let stream = match stream_name {
            "_" => {
                let stream = db_util::get_current_stream(&mut *transaction, channel_id)
                    .await.map_err(|e| {
                        eprintln!("get current stream {:?}", e);
                        HandlerError::with_message("DB error".into())
                    })?;
                
                match stream {
                    Some(stream) => stream,
                    None => {
                        return Ok("No active stream".into())
                    }
                }
            }
            stream_name => {
                let (stream_name, query_server_id) = match cross_server {
                    true => (stream_name, None),
                    false => {
                        match stream_name.rsplit_once('|') {
                            Some((stream_name, server_str)) => {
                                let server_id: u64 = server_str.parse().map_err(
                                    |_| {
                                        HandlerError::with_message(format!("invalid server id {}", server_str))
                                    }
                                )?;
                                (stream_name, Some(server_id))
                            }
                            None => (stream_name, Some(server_id.0)),
                        }
                    }
                };
                let stream = db_util::get_stream_by_name(&mut *transaction, stream_name, query_server_id, Some(server_id.0))
                    .await.map_err(|e| {
                        eprintln!("get stream {:?}", e);
                        HandlerError::with_message("DB error".into())
                    })?;
                
                match stream {
                    Some(stream) => stream,
                    None => {
                        return Ok("Stream not found, try `!tags c <stream>` and \
                        ensuring youtube links are https, not shortened and \
                        don't have extra query parameters (e.g. ?t=1)".into())
                    }
                }
            }
        };
        let stream_name = match as_name {
            Some(name) => name,
            None => &stream.name
        };
        
        use sqlx::Arguments;
        let mut query_args = sqlx::postgres::PgArguments::default();
        
        let selector = if cross_server {
            query_args.add(&stream.name);
            "streams.name = $1"
        } else {
            query_args.add(stream.id);
            "streams.id = $1"
        };
        query_args.add(&stream.name);
        
        let tags: Vec<(String, DateTime, i32, i64, f64)> = sqlx::query_as_with(&format!(r#"
            SELECT tags."name", tags."time", tags.votes, tags."user",
                EXTRACT(EPOCH FROM
                    COALESCE(SUM(tags.tag_offsets."offset"), interval '0')
                )::FLOAT8 "adj"
            FROM tags.streams, tags.tags
            LEFT JOIN tags.tag_offsets ON tags.id = tag_offsets.tag
            WHERE
                {} and
                --streams.id = $2 and
                --streams.name = $1 and
                tags.stream = streams.id and
                tags.deleted = false
            GROUP BY tags.id
            ORDER BY tags."time"
        "#, selector), query_args)
            // .bind(&stream.name)
            // .bind(None::<&str>)
            // .bind(stream.id)
            // .bind(None::<i32>)
            .fetch_all(&mut *transaction).await.map_err(|e| {
                eprintln!("get tags {:?}", e);
                HandlerError::with_message("DB error".into())
            })?;
        
        let offsets: Vec<(f64, Option<f64>, f64)> = sqlx::query_as(r#"
            SELECT
                EXTRACT(EPOCH FROM position)::FLOAT8,
                EXTRACT(EPOCH FROM "end")::FLOAT8,
                EXTRACT(EPOCH FROM "offset")::FLOAT8
            FROM tags.stream_offsets
            WHERE stream = $1
            ORDER BY "order"
        "#)
            .bind(stream.id)
            .fetch_all(&mut *transaction).await.map_err(|e| {
                eprintln!("get offsets {:?}", e);
                HandlerError::with_message("DB error".into())
            })?;
        
        let mut out = String::new();
        
        // let jst_time = stream.start_time.assume_utc().to_offset(time::UtcOffset::hours(9));
        let offset = chrono::FixedOffset::east_opt(9 * 60 * 60).unwrap();
        let jst_time = stream.start_time.and_utc().with_timezone(&offset);
        // let time_disp = jst_time.lazy_format("%T JST");
        let start_timestamp = jst_time.timestamp();
        let time_disp = format!("<t:{}>", start_timestamp);
        
        let probably_link = stream.name.starts_with("http://") || stream.name.starts_with("https://");
        let stream_display_name = if probably_link {
            std::borrow::Cow::Borrowed(stream.name.as_str())
        } else {
            escape_embed(&stream.name)
        };
        
        let video_id = VideoId::extract(stream_name);
        
        struct Adjusted {
            tag_name: String,
            delta: Duration,
            tag_votes: i32,
        }
        let mut adjusted_tags = Vec::new();
        for (tag_name, tag_time, tag_votes, tag_user, tag_adj) in tags.into_iter() {
            if own_tags && from_i(tag_user) != user_id {
                continue
            }
            
            let mut delta: Duration = tag_time - stream.start_time;
            
            // delta += Duration::seconds_f64(tag_adj);
            // delta += Duration::microseconds((tag_adj / 1000000.0) as i64);
            delta += seconds_f64(tag_adj);
            delta -= Duration::seconds(20);
            
            for (pos, end, offset) in offsets.iter() {
                if delta >= seconds_f64(*pos) && end.map_or(true, |x| delta <= seconds_f64(x)) {
                    delta += seconds_f64(*offset);
                }
            }
            
            if limit_time && (delta < Duration::seconds(0) || delta > Duration::seconds(12*60*60)) {
                continue
            }
            
            adjusted_tags.push(Adjusted {
                tag_name: tag_name,
                delta: delta,
                tag_votes: tag_votes,
            });
        }
        
        adjusted_tags.sort_by(|a, b| a.delta.cmp(&b.delta));
        
        let per_min_str = if let Some(tag) = adjusted_tags.last() {
            let tags_per_min = adjusted_tags.len() as f64 / (tag.delta.num_milliseconds() as f64 / 1000.0 / 60.0);
            format!(" ({:.1}/min)", tags_per_min)
        } else {
            "".into()
        };
        // out.push_str(&format!("{} start time: {} {} tags{}\n",
        out.push_str(&format!("{} {} {} tags{}\n",
            stream_display_name.as_ref(), time_disp, adjusted_tags.len(), per_min_str));
        
        if format_csv {
            out.push_str("name,votes,offset\n")
        }
        
        for tag in adjusted_tags {
            if info_only {
                
            } else if format_csv {
                format_tag_csv(&mut out, &tag.tag_name, tag.tag_votes, stream_name, tag.delta).map_err(|e| {
                    eprintln!("format csv {:?}", e);
                    HandlerError::with_message("error".into())
                })?;
            } else if format_yt {
                format_tag_yt(&mut out, &tag.tag_name, tag.tag_votes, stream_name, tag.delta);
            } else {
                format_tag_standard(&mut out, &video_id, &tag.tag_name, tag.tag_votes, stream_name, tag.delta);
            }
        }
        
        transaction.commit().await.map_err(|e| {
            eprintln!("transaction commit {:?}", e);
            HandlerError::with_message("DB error".into())
        })?;
        
        let res = HandlerResponse::wrapped_embed(Some("Tags"), &out);
        
        Ok(res)
    }
    
    fn help_info_simple(&self) -> Option<SimpleHelpInfo> {
        Some((
            "!tags [flags] [stream name]",
            "lists everyone tags for the active stream. \
            Please do it in a dedicated bot channel as this can be very long. \
            `!tags own _` lists your own tags. \
            `!tags own <stream URL>` lists your own tags for the given stream.",
        ).into())
    }
}

#[derive(Debug)]
pub struct TagHandler {
    pub vote_emoji: String,
    pub delete_emoji: String,
    pub pool: PgPool,
}

pub fn parse_tag_message(msg: &str) -> Option<&str> {
    let tag_name = if msg.starts_with("!") {
        let mut args = msg.splitn(2, " ");
        
        let command = args.next();
        if command != Some("!tag") && command != Some("!t") {
            // return Ok(().into())
            return None
        }
        
        let tag_name: &str = match args.next() {
            Some(tag_name) => tag_name,
            None => {
                // return Err(HandlerError::with_message("requires tag name".into()))
                return None
            }
        };
        
        tag_name
    } else if msg.starts_with("``") && !msg.starts_with("```") && !msg[2..].contains("``") {
        let tag_name = msg[2..].trim();
        tag_name
    } else if msg.starts_with("`") && !msg.chars().skip(1).any(|c| c == '`') {
        let tag_name = msg[1..].trim();
        tag_name
    } else {
        // return Ok(().into())
        return None
    };
    
    if tag_name.len() == 0 {
        // return Ok(().into())
        return None
    }
    
    Some(tag_name)
}

#[async_trait]
impl Handler for TagHandler {
    async fn handle_message(&self, msg: &Message) -> HandlerResult {
        
        // let tag_time = time::OffsetDateTime::now_utc();
        let tag_time = chrono::offset::Utc::now().naive_utc();
        
        let tag_name = if msg.content.starts_with("!") {
            let mut args = msg.content.splitn(2, " ");
            
            let command = args.next();
            if command != Some("!tag") && command != Some("!t") {
                return Ok(().into())
            }
            
            let tag_name: &str = match args.next() {
                Some(tag_name) => tag_name,
                None => {
                    return Err(HandlerError::with_message("requires tag name".into()))
                }
            };
            
            tag_name
        } else if msg.content.starts_with("``") && !msg.content.starts_with("```") && !msg.content[2..].contains("``") {
            let tag_name = msg.content[2..].trim();
            tag_name
        } else if msg.content.starts_with("`") && !msg.content.chars().skip(1).any(|c| c == '`') {
            let tag_name = msg.content[1..].trim();
            tag_name
        } else {
            return Ok(().into())
        };
        
        if tag_name.len() == 0 {
            return Ok(().into())
        }
        
        let channel_id = msg.channel_id.0;
        
        let mut transaction = self.pool.begin().await.map_err(|e| {
            eprintln!("transaction begin {:?}", e);
            HandlerError::with_message("DB error".into())
        })?;
        
        let stream = crate::db_util::get_current_stream(&mut *transaction, channel_id)
            .await.map_err(|e| {
                eprintln!("get current stream {:?}", e);
                HandlerError::with_message("DB error".into())
            })?;
        
        let pg_stream_id = match stream {
            Some(stream) => stream.id,
            None => {
                return Ok("No active stream".into())
            }
        };
        
        sqlx::query(r#"
            INSERT INTO tags.tags
                   (stream, "name", "time", "server", "user", message_id)
            VALUES ($1,     $2,     $3,     $4,       $5,     $6)
        "#)
            .bind(pg_stream_id)
            .bind(tag_name)
            .bind(tag_time)
            .bind(msg.guild_id.as_ref().map(|x| to_i(x.0)))
            .bind(to_i(msg.author.id.0))
            .bind(to_i(msg.id.0))
            .execute(&mut *transaction).await.map_err(|e| {
                eprintln!("insert tag {:?}", e);
                HandlerError::with_message("DB error".into())
            })?;
        
        transaction.commit().await.map_err(|e| {
            eprintln!("transaction commit {:?}", e);
            HandlerError::with_message("DB error".into())
        })?;
        
        let mut res: HandlerResponse = ().into();
        // res.add_reaction("👌");
        // res.add_reaction("🅱️ 🅱");
        // res.add_reaction("🅱");
        res.add_reaction(&self.vote_emoji);
        res.add_reaction(&self.delete_emoji);
        Ok(res)
    }
    
    async fn handle_reaction_add_simple(&self, reaction: &MessageReactionAdd) {
        let message_id = reaction.message_id.0;
        // let channel_id = reaction.channel_id.0;
        let user_id = reaction.user_id.0;
        
        match &reaction.emoji.name {
            emoji if emoji == &self.vote_emoji => {
                let res = sqlx::query(r#"
                    UPDATE tags.tags
                        SET votes = votes + 1
                        WHERE message_id = $1
                "#)
                    .bind(to_i(message_id))
                    .execute(&self.pool).await;
                if let Err(err) = res {
                    eprintln!("votes up {:?}", err);
                }
            }
            emoji if emoji == &self.delete_emoji => {
                let res = sqlx::query(r#"
                    UPDATE tags.tags
                        SET deleted = true
                        WHERE message_id = $1 AND "user" = $2
                "#)
                    .bind(to_i(message_id))
                    .bind(to_i(user_id))
                    .execute(&self.pool).await;
                if let Err(err) = res {
                    eprintln!("delete {:?}", err);
                }
            }
            _ => (),
        }
    }
    
    async fn handle_reaction_remove_simple(&self, reaction: &MessageReactionRemove) {
        let message_id = reaction.message_id.0;
        // let channel_id = reaction.channel_id.0;
        let user_id = reaction.user_id.0;
        
        match &reaction.emoji.name {
            emoji if emoji == &self.vote_emoji => {
                let res = sqlx::query(r#"
                    UPDATE tags.tags
                        SET votes = votes - 1
                        WHERE message_id = $1
                "#)
                    .bind(to_i(message_id))
                    .execute(&self.pool).await;
                if let Err(err) = res {
                    eprintln!("votes down {:?}", err);
                }
            }
            emoji if emoji == &self.delete_emoji => {
                let res = sqlx::query(r#"
                    UPDATE tags.tags
                        SET deleted = false
                        WHERE message_id = $1 AND "user" = $2
                "#)
                    .bind(to_i(message_id))
                    .bind(to_i(user_id))
                    .execute(&self.pool).await;
                if let Err(err) = res {
                    eprintln!("delete {:?}", err);
                }
            }
            _ => (),
        }
    }
    
    fn help_info_simple(&self) -> Option<SimpleHelpInfo> {
        Some((
            "!tag <free text>",
            "creates a timestamp on the active stream with <free text> as description. Korotagger will acknowledge with a react.",
        ).into())
    }
}

#[derive(Debug)]
pub struct AdjustHandler {
    pub pool: PgPool,
}

#[async_trait]
impl Handler for AdjustHandler {
    async fn handle_command(&self, args: &[&str], msg: &Message) -> HandlerResult {
        // let channel_id = msg.channel_id.0;
        let user = msg.author.id.0;
        let channel_id = msg.channel_id.0;
        
        let adjust: i64 = args.get(0)
            .and_then(|x| x.parse().ok())
            .ok_or_else(|| HandlerError::with_message("Invalid argument.".into()))?;
        
        let mut transaction = self.pool.begin().await.map_err(|e| {
            eprintln!("transaction begin {:?}", e);
            HandlerError::with_message("DB error".into())
        })?;
        
        let (tag_id,): (i32,) = sqlx::query_as(r#"
            SELECT tags.id
            FROM tags.tags
            WHERE
                "user" = $1 and
                stream = (
                    select selected_streams.stream
                    from config.selected_streams
                    where
                        channel = $2
                )
            ORDER BY "time" DESC
            LIMIT 1
        "#)
            .bind(to_i(user))
            .bind(to_i(channel_id))
            .fetch_optional(&mut *transaction).await.map_err(|e| {
                eprintln!("get tag {:?}", e);
                HandlerError::with_message("DB error".into())
            })?
            .ok_or_else(|| HandlerError::with_message("No past tag found.".into()))?;
        
        sqlx::query(r#"
            INSERT INTO tags.tag_offsets
                   ("order", tag, "offset")
            VALUES (0,       $1,  $2)
        "#)
            .bind(tag_id)
            .bind(Duration::seconds(adjust))
            .execute(&mut *transaction).await.map_err(|e| {
                eprintln!("insert tag offset {:?}", e);
                HandlerError::with_message("DB error".into())
            })?;
        
        transaction.commit().await.map_err(|e| {
            eprintln!("transaction commit {:?}", e);
            HandlerError::with_message("DB error".into())
        })?;
        
        let mut res: HandlerResponse = ().into();
        // res.add_reaction("👌");
        // res.add_reaction("🅱️ 🅱");
        // res.add_reaction("🅱");
        // res.add_reaction(&self.vote_emoji);
        res.add_reaction("👍");
        Ok(res)
    }
}
