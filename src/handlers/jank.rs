
use async_trait::async_trait;

// #[macro_use] use crate::handlers;
use super::{ Handler, HandlerResult, HandlerError, Command };
// use discord_lib::discord::Message;
// use discord_lib::gateway::{ MessageReactionAdd, MessageReactionRemove };
// use crate::extract_id;

// use crate::DB;
// use crate::{ State, Tag };
// use chrono::Utc;
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
        
        // {
        //     let mut data = DB.borrow_data_mut().unwrap();
            
        //     let stream_a = match data.streams.get(stream_name_a) {
        //         Some(x) => x,
        //         None => return Err(HandlerError::with_message("Stream a not found".into()))
        //     };
            
        //     let tags: Vec<Tag> = stream_a.tags.iter().map(|tag| tag.clone()).collect();
            
        //     let stream_b = match data.streams.get_mut(stream_name_b) {
        //         Some(x) => x,
        //         None => return Err(HandlerError::with_message("Stream b not found".into()))
        //     };
        //     for tag in tags {
        //         stream_b.tags.push(tag);
        //     }
        // }
        // DB.async_save_data().await.unwrap();
        
        let mut transaction = self.pool.begin().await.map_err(|e| {
            eprintln!("transaction begin {:?}", e);
            HandlerError::with_message("DB error".into())
        })?;
        
        let in_stream = db_util::get_stream_by_name(&mut transaction, stream_name_a, Some(server_id.0), None)
            .await.map_err(|e| {
                eprintln!("get stream a {:?}", e);
                HandlerError::with_message("DB error".into())
            })?
            .ok_or_else(|| HandlerError::with_message("Stream a not found".into()))?;
        
        let out_stream = db_util::get_stream_by_name(&mut transaction, stream_name_b, Some(server_id.0), None)
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
            .execute(&mut transaction).await.map_err(|e| {
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

// #[derive(Debug)]
// pub struct RecreateHandler {
    
// }

// #[async_trait]
// impl Handler for RecreateHandler {
//     async fn handle_command_b(&self, command: Command<'_>) -> HandlerResult {
//         command.require_admin().await?;
        
//         let stream_name_a = *command.args.get(0).ok_or_else(|| HandlerError::with_message("Incorrect args.".into()))?;
//         let stream_name_b = *command.args.get(1).ok_or_else(|| HandlerError::with_message("Incorrect args.".into()))?;
        
//         {
//             let mut data = DB.borrow_data_mut().unwrap();
            
//             let stream_a = match data.streams.get(stream_name_a) {
//                 Some(x) => x,
//                 None => return Err(HandlerError::with_message("Stream a not found".into()))
//             };
            
//             let stream_b = stream_a.clone();
//             // let tags: Vec<Tag> = stream_a.tags.iter().map(|tag| tag.clone()).collect();
            
            
//             match data.streams.get_mut(stream_name_b) {
//                 Some(_) => return Err(HandlerError::with_message("Stream b already exists".into())),
//                 // None => return Err(HandlerError::with_message("Stream b not found".into()))
//                 None => ()
//             }
            
            
//             data.streams.insert(stream_name_b.into(), stream_b);
//             // for tag in tags {
//             //     stream_b.tags.push(tag);
//             // }
//         }
//         DB.async_save_data().await.unwrap();
        
//         Ok("_".into())
//     }
// }
