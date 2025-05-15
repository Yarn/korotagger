
use std::sync::Arc;
use std::panic::AssertUnwindSafe;
use futures::prelude::*;

use discord_lib::discord::Snowflake;
use discord_lib::gateway::Event as E;
use crate::to_i;

use crate::DiscordState;

pub async fn handle_event(
    event: E,
    state: Arc<DiscordState>,
    self_id: Option<Snowflake>,
) {
    match event {
        E::MessageCreate(msg) => {
            // ignore messages from self
            if let Some(ref self_id) = self_id {
                if &msg.author.id == self_id {
                    // continue;
                    return;
                }
            }
            
            let future = super::handle_message_create(state, msg);
            tokio::task::spawn(future);
            // if content == "!help" {
            //     let text = help_text::get_help_text(&*handlers, &command_aliases, "!");
                
            //     let send_msg: discord_lib::send_message::NewMessage = text.into();
                
            //     let to = msg.channel_id.clone();
            //     let fut = send_handle.send(to, &send_msg);
            //     if let Err(err) = fut.await {
            //         dbg!("Failed to send message: {:?}", err);
            //         continue
            //     }
            // } else if content == "!voice" {
            //     // let msg = r#"
            //     //     {
            //     //         "op": 4,
            //     //         "d": {
            //     //             "guild_id": "-",
            //     //             "channel_id": "-",
            //     //             "self_mute": false,
            //     //             "self_deaf": false
            //     //         }
            //     //     }
            //     // "#;
                
            //     // discord_obj.send_gateway_raw(msg).await.unwrap();
        }
        E::MessageUpdate(msg) => {
            tokio::task::spawn(async move {
                let res = AssertUnwindSafe((|| async {
                    let content = msg.content.as_str();
                    
                    let tag_name = crate::handlers::tagging::parse_tag_message(content);
                    let tag_name = match tag_name {
                        Some(x) => x,
                        None => return,
                    };
                    
                    let message_id: u64 = msg.id.0;
                    
                    let res = sqlx::query(r#"
                        UPDATE tags.tags
                            SET "name" = $2
                            WHERE message_id = $1
                    "#)
                        .bind(to_i(message_id))
                        .bind(tag_name)
                        .execute(&state.pool).await;
                    if let Err(err) = res {
                        eprintln!("edit tag {:?}", err);
                    }
                    
                    // pool;
                })()).catch_unwind().await;
                if let Err(err) = res {
                    dbg!(err);
                }
            });
        }
        E::MessageDelete(msg) => {
            tokio::task::spawn(async move {
                let res = AssertUnwindSafe((|| async {
                    let message_id: u64 = msg.id.0;
                    
                    let res = sqlx::query(r#"
                        DELETE FROM tags.tags
                            WHERE message_id = $1
                    "#)
                        .bind(to_i(message_id))
                        .execute(&state.pool).await;
                    if let Err(err) = res {
                        eprintln!("edit tag {:?}", err);
                    }
                })()).catch_unwind().await;
                if let Err(err) = res {
                    dbg!(err);
                }
            });
        }
        E::MessageReactionAdd(reaction) => {
            if let Some(ref self_id) = self_id {
                if &reaction.user_id == self_id {
                    return;
                }
            }
            // eprintln!("reaction {:#?}", reaction);
            tokio::task::spawn(async move {
                for handler in state.handlers.values() {
                    match AssertUnwindSafe(handler.handle_reaction_add_simple(&reaction)).catch_unwind().await {
                        Ok(()) => {}
                        Err(err) => {
                            dbg!(handler, err);
                        }
                    }
                }
            });
        }
        E::MessageReactionRemove(reaction) => {
            if let Some(ref self_id) = self_id {
                if &reaction.user_id == self_id {
                    return;
                }
            }
            // eprintln!("reaction remove {:?}", reaction);
            
            tokio::task::spawn(async move {
                for handler in state.handlers.values() {
                    match AssertUnwindSafe(handler.handle_reaction_remove_simple(&reaction)).catch_unwind().await {
                        Ok(()) => {}
                        Err(err) => {
                            dbg!(handler, err);
                        }
                    }
                }
            });
        }
        E::GuildCreate(guild) => {
            // dbg!("guild create", guild.id);
            let mut server_state = state.server_state.write().await;
            server_state.update_guild(&guild);
            std::mem::drop(server_state);
            
            state.save_server_state().await;
        }
        E::GuildUpdate(guild) => {
            // dbg!("guild update", guild.id);
            let mut server_state = state.server_state.write().await;
            server_state.update_guild(&guild);
            std::mem::drop(server_state);
            state.save_server_state().await;
        }
        E::GuildDelete(guild) => {
            // dbg!("guild delete", guild.id);
            let mut server_state = state.server_state.write().await;
            server_state.delete_guild(guild.id);
            std::mem::drop(server_state);
            state.save_server_state().await;
        }
        E::GuildRoleCreate(role) => {
            // dbg!("guild role create", role.guild_id);
            let mut server_state = state.server_state.write().await;
            server_state.update_role(role.guild_id, &role.role);
            std::mem::drop(server_state);
            state.save_server_state().await;
        }
        E::GuildRoleUpdate(role) => {
            // dbg!("guild role update", role.guild_id);
            let mut server_state = state.server_state.write().await;
            server_state.update_role(role.guild_id, &role.role);
            std::mem::drop(server_state);
            state.save_server_state().await;
        }
        E::GuildRoleDelete(role) => {
            // dbg!("guild role delete", role.guild_id);
            let mut server_state = state.server_state.write().await;
            server_state.delete_role(role.guild_id, role.role_id);
            std::mem::drop(server_state);
            state.save_server_state().await;
        }
        E::ChannelCreate(chan) => {
            // dbg!("channel create", chan.guild_id);
            let mut server_state = state.server_state.write().await;
            server_state.update_channel(&chan);
            std::mem::drop(server_state);
            state.save_server_state().await;
        }
        E::ChannelUpdate(chan) => {
            // dbg!("channel update", chan.guild_id);
            let mut server_state = state.server_state.write().await;
            server_state.update_channel(&chan);
            std::mem::drop(server_state);
            state.save_server_state().await;
        }
        E::ChannelDelete(chan) => {
            // dbg!("channel delete", chan.guild_id);
            if let Some(guild) = chan.guild_id {
                let mut server_state = state.server_state.write().await;
                server_state.delete_channel(guild, chan.id);
                std::mem::drop(server_state);
                state.save_server_state().await;
            }
        }
        _ => ()
    }
}
