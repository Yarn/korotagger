
use async_trait::async_trait;

use super::{ Handler, HandlerResult, HandlerError, Command };

use sqlx::PgPool;
use crate::db_util;

#[derive(Debug)]
pub struct CopyTagsHandler {
    pub pool: PgPool,
}

#[async_trait]
impl Handler for CopyTagsHandler {
    async fn handle_command_b(&self, command: Command<'_>) -> HandlerResult {
        command.require_admin().await?;
        
        let stream_name_a = *command.args.get(0).ok_or_else(|| HandlerError::with_message("Incorrect args.".into()))?;
        let stream_name_b = *command.args.get(1).ok_or_else(|| HandlerError::with_message("Incorrect args.".into()))?;
        let ref server_id = command.message.guild_id.as_ref()
            .ok_or(HandlerError::with_message("Not in a server".into()))?;
        
        let mut transaction = self.pool.begin().await.map_err(|e| {
            eprintln!("transaction begin {:?}", e);
            HandlerError::with_message("DB error".into())
        })?;
        
        let (stream_name_a, server_a) = match stream_name_a.rsplit_once('|') {
            Some((name, server)) => {
                let server: u64 = if server == "" {
                    server_id.0
                } else {
                    server.parse()
                        .map_err(|_err| HandlerError::with_message(format!("Invalid server id {}", server)))?
                };
                (name, server)
            }
            None => {
                (stream_name_a, server_id.0)
            }
        };
        
        let in_stream = db_util::get_stream_by_name(&mut *transaction, stream_name_a, Some(server_a), None)
            .await.map_err(|e| {
                eprintln!("get stream a {:?}", e);
                HandlerError::with_message("DB error".into())
            })?
            .ok_or_else(|| HandlerError::with_message("Stream a not found".into()))?;
        
        let out_stream = db_util::get_stream_by_name(&mut *transaction, stream_name_b, Some(server_id.0), None)
            .await.map_err(|e| {
                eprintln!("get stream b {:?}", e);
                HandlerError::with_message("DB error".into())
            })?
            .ok_or_else(|| HandlerError::with_message("Stream b not found".into()))?;
        
        sqlx::query(r#"
            INSERT INTO tags.tags
                  (stream, "name", "time", "server", "user", message_id, "deleted")
            SELECT $1,     "name", "time", "server", "user", message_id, "deleted"
                FROM tags.tags
                WHERE stream = $2
        "#)
            .bind(out_stream.id)
            .bind(in_stream.id)
            .execute(&mut *transaction).await.map_err(|e| {
                eprintln!("insert copy tags {:?}", e);
                HandlerError::with_message("DB error".into())
            })?;
        
        transaction.commit().await.map_err(|e| {
            eprintln!("transaction commit {:?}", e);
            HandlerError::with_message("DB error".into())
        })?;
        
        Ok("_".into())
    }
}
