
use async_trait::async_trait;

use super::{ Handler, HandlerResult, HandlerError, Command };
use crate::get_mention_ids;

// use crate::DB;
// use std::collections::BTreeSet;

use sqlx::PgPool;
use crate::{ from_i, to_i };

#[derive(Debug)]
pub struct SubscribeHandler {
    pub pool: PgPool,
}

#[async_trait]
impl Handler for SubscribeHandler {
    async fn handle_command_b(&self, command: Command<'_>) -> HandlerResult {
        command.require_admin().await?;
        
        let op: &str = command.args.get(0)
            .map(|x| *x)
            .ok_or(HandlerError::with_message("Missing param".into()))?;
        
        match op {
            "add" => {
                let yt_channel_id = command.args.get(1)
                    .map(|x| *x)
                    .ok_or(HandlerError::with_message("Missing param".into()))?;
                
                let channel_id = command.message.channel_id.0;
                
                // {
                //     let mut data = DB.borrow_data_mut().unwrap();
                    
                //     data.subscriptions
                //         .entry(channel_id)
                //         .or_insert_with(|| Vec::new())
                //         .push(yt_channel_id.into())
                //         ;
                // }
                // DB.async_save_config().await.unwrap();
                
                sqlx::query(r#"
                    INSERT INTO config.subscriptions (channel, sub_id, "type") VALUES (
                        $1,
                        $2,
                        'youtube'
                    )
                    ON CONFLICT DO NOTHING;
                "#)
                    .bind(to_i(channel_id))
                    .bind(yt_channel_id)
                    .execute(&self.pool).await.map_err(|e| {
                        eprintln!("add sub {:?}", e);
                        HandlerError::with_message("DB error".into())
                    })?;
                
                Ok("sub set".into())
            }
            "watch_channel" => {
                let target_channel_id = command.args.get(1)
                    .map(|x| *x)
                    .ok_or(HandlerError::with_message("Missing param".into()))?;
                
                let channel_id = command.message.channel_id.0;
                
                sqlx::query(r#"
                    INSERT INTO config.subscriptions (channel, sub_id, "type") VALUES (
                        $1,
                        $2,
                        'watch_channel'
                    )
                    ON CONFLICT DO NOTHING;
                "#)
                    .bind(to_i(channel_id))
                    .bind(target_channel_id)
                    .execute(&self.pool).await.map_err(|e| {
                        eprintln!("add sub {:?}", e);
                        HandlerError::with_message("DB error".into())
                    })?;
                
                Ok("watch sub set".into())
            }
            "clear" => {
                
                let channel_id = command.message.channel_id.0;
                
                // {
                //     let mut data = DB.borrow_data_mut().unwrap();
                    
                //     data.subscriptions.remove(&channel_id);
                // }
                // DB.async_save_data().await.unwrap();
                
                sqlx::query(r#"
                    DELETE FROM config.subscriptions
                        WHERE channel = $1
                "#)
                    .bind(to_i(channel_id))
                    .execute(&self.pool).await.map_err(|e| {
                        eprintln!("add sub {:?}", e);
                        HandlerError::with_message("DB error".into())
                    })?;
                
                Ok("subs cleared".into())
            }
            _ => {
                Err(HandlerError::with_message("Invalid op".into()))
            }
        }
    }
}

#[derive(Debug)]
pub struct ManageAdminHandler {
    pub pool: PgPool,
}

#[async_trait]
impl Handler for ManageAdminHandler {
    async fn handle_command_b(&self, command: Command<'_>) -> HandlerResult {
        command.require_admin().await?;
        
        let msg = command.message;
        let op: &str = command.args.get(0)
            .map(|x| *x)
            .ok_or(HandlerError::with_message("Missing param".into()))?;
        
        match op {
            "add" => {
                let new_ids: Vec<u64> = match get_mention_ids(&msg) {
                    Ok(ids) => ids,
                    Err(msg) => {
                        // reply!(msg);
                        // continue
                        return Ok(msg.into())
                    }
                };
                
                let guild_id: u64 = match msg.guild_id {
                    Some(ref guild_id) => {
                        guild_id.0
                    }
                    None => {
                        // this should never happen after require_admin!()
                        return Err(HandlerError::with_message("Must be used within guild".into()))
                    }
                };
                
                let mut out: String = "_".into();
                // {
                //     let mut data = DB.config_state.borrow_data_mut().unwrap();
                    
                //     let admins = data.admin_perm
                //         .entry(guild_id)
                //         .or_insert_with(|| BTreeSet::new());
                    
                //     for new_id in new_ids {
                //         admins.insert(new_id);
                //         out.push_str(&format!(" {}", new_id));
                //     }
                // }
                // DB.async_save_config().await.unwrap();
                
                let mut transaction = self.pool.begin().await.map_err(|e| {
                    eprintln!("transaction begin {:?}", e);
                    HandlerError::with_message("DB error".into())
                })?;
                
                for new_id in new_ids {
                    sqlx::query(r#"
                        INSERT INTO config.server_admins (server, "group", readable) VALUES (
                            $1,
                            $2,
                            NULL
                        )
                        ON CONFLICT DO NOTHING;
                    "#)
                        .bind(to_i(guild_id))
                        .bind(to_i(new_id))
                        .execute(&mut transaction).await.map_err(|e| {
                            eprintln!("insert server admin {:?}", e);
                            HandlerError::with_message("DB error".into())
                        })?;
                    out.push_str(&format!(" {}", new_id));
                }
                
                transaction.commit().await.map_err(|e| {
                    eprintln!("transaction commit {:?}", e);
                    HandlerError::with_message("DB error".into())
                })?;
                
                Ok(out.into())
            }
            "rem" => {
                let rem_ids: Vec<u64> = match get_mention_ids(&msg) {
                    Ok(ids) => ids,
                    Err(msg) => {
                        return Ok(msg.into())
                    }
                };
                
                let guild_id: u64 = match msg.guild_id {
                    Some(ref guild_id) => {
                        guild_id.0
                    }
                    None => {
                        // this should never happen after require_admin!()
                        return Err(HandlerError::with_message("Must be used within guild".into()))
                    }
                };
                
                // {
                //     let mut data = DB.config_state.borrow_data_mut().unwrap();
                    
                //     let admins = data.admin_perm
                //         .entry(guild_id)
                //         .or_insert_with(|| BTreeSet::new());
                    
                //     for rem_id in rem_ids {
                //         admins.remove(&rem_id);
                //     }
                // }
                // DB.async_save_config().await.unwrap();
                
                // let mut transaction = self.pool.begin().await.map_err(|e| {
                //     eprintln!("transaction begin {:?}", e);
                //     HandlerError::with_message("DB error".into())
                // })?;
                
                for rem_id in rem_ids {
                    sqlx::query(r#"
                        DELETE FROM config.server_admins
                            WHERE server = $1 and "group" = $2
                    "#)
                        .bind(to_i(guild_id))
                        .bind(to_i(rem_id))
                        .execute(&self.pool).await.map_err(|e| {
                            eprintln!("add sub {:?}", e);
                            HandlerError::with_message("DB error".into())
                        })?;
                }
                
                // transaction.commit().await.map_err(|e| {
                //     eprintln!("transaction commit {:?}", e);
                //     HandlerError::with_message("DB error".into())
                // })?;
                
                Ok("_".into())
            }
            "list" => {
                let guild_id: u64 = match msg.guild_id {
                    Some(ref guild_id) => {
                        guild_id.0
                    }
                    None => {
                        // this should never happen after require_admin!()
                        return Err(HandlerError::with_message("Must be used within guild".into()))
                    }
                };
                
                let mut out = "_".to_string();
                
                // {
                //     let data = DB.config_state.borrow_data().unwrap();
                    
                //     if let Some(admins) = data.admin_perm.get(&guild_id) {
                //         // .entry(guild_id)
                //         // .or_insert_with(|| BTreeSet::new());
                    
                //         for admin in admins.iter() {
                //             out.push_str(&format!(" {:?}", admin));
                //         }
                //     }
                // }
                
                let admins: Vec<(i64,)> = sqlx::query_as(r#"
                    SELECT "group"
                    FROM config.server_admins
                    WHERE server = $1
                "#)
                    .bind(to_i(guild_id))
                    .fetch_all(&self.pool).await.map_err(|e| {
                        eprintln!("get admins {:?}", e);
                        HandlerError::with_message("DB error".into())
                    })?;
                
                for (admin,) in admins {
                    out.push_str(&format!(" {:?}", from_i(admin)));
                }
                
                Ok(out.into())
            }
            _ => {
                Err(HandlerError::with_message("Invalid op".into()))
            }
        }
    }
}
