
use sqlx::prelude::Executor;
use sqlx::Postgres;
use sqlx::postgres::PgArguments;

use crate::to_i;

type DateTime = chrono::NaiveDateTime;

#[allow(dead_code)]
pub struct Select<'a> {
    fields: Vec<&'a str>,
    table: &'a str,
    conditions: Vec<&'a str>,
    args: PgArguments,
}

pub struct Stream {
    pub id: i32,
    pub name: String,
    pub start_time: DateTime,
}

pub async fn get_stream_by_name<'c, E>(exec: E, name: &str, server_id: Option<u64>, server_id_hint: Option<u64>) -> Result<Option<Stream>, sqlx::Error>
    where
        E: Executor<'c, Database = Postgres>
{
    let server_query = server_id
        .map(|x| format!(" and (streams.has_server = true and streams.server = {})", x))
        .unwrap_or("".to_string());
    
    let selected_stream: Option<(String, i32, DateTime)> = sqlx::query_as(&format!(r#"
        SELECT streams.name, streams.id, streams.start_time FROM tags.streams
        WHERE streams."name" = $1{}
        ORDER BY has_server DESC,
            "server" = $2 DESC NULLS LAST
    "#, server_query))
        .bind(name)
        .bind(server_id_hint.map(|x| to_i(x)))
        .fetch_optional(exec).await
        ?;
    
    let stream = match selected_stream {
        Some((stream_name, pg_stream_id, start_time)) => {
            Stream {
                id: pg_stream_id,
                name: stream_name,
                start_time: start_time,
            }
        },
        None => {
            return Ok(None);
        }
    };
    
    Ok(Some(stream))
    // panic!()
}

pub async fn get_current_stream<'c, E>(exec: E, channel_id: u64) -> Result<Option<Stream>, sqlx::Error>
    where
        E: Executor<'c, Database = Postgres>
{
    let selected_stream: Option<(String, i32, DateTime)> = sqlx::query_as(r#"
        SELECT streams.name, streams.id, streams.start_time FROM config.selected_streams, tags.streams
        WHERE channel = $1 and stream = streams.id;
    "#)
        .bind(to_i(channel_id))
        .fetch_optional(exec).await
        ?;
    
    let stream = match selected_stream {
        Some((stream_name, pg_stream_id, start_time)) => {
            Stream {
                id: pg_stream_id,
                name: stream_name,
                start_time: start_time,
            }
        },
        None => {
            return Ok(None);
        }
    };
    
    Ok(Some(stream))
    // panic!()
}
