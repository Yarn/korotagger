
use async_trait::async_trait;

#[allow(unused_imports)]
use super::{ Handler, HandlerResult, HandlerResponse, HandlerError, Command };
#[allow(unused_imports)]
use discord_lib::discord::Message;
use discord_lib::send_message::NewMessage;
// use crate::permissions::is_msg_from_admin;

#[derive(Debug)]
pub struct TestHandler {
    
}

#[async_trait]
impl Handler for TestHandler {
    async fn handle_command_b(&self, command: Command<'_>) -> HandlerResult {
        let args = command.args;
        
        match args.get(0).map(|x| *x) {
            Some("echo") => {
                return Ok("!echo".into())
            }
            Some("embed") => {
                // let is_admin = is_msg_from_admin(&command.message).unwrap();
                let is_admin = false;
                
                let text = format!("admin {:?}", is_admin);
                
                let resp = NewMessage::embed_temp(text);
                
                return Ok(resp.into())
            }
            Some("react") => {
                let mut resp: HandlerResponse = ().into();
                resp.add_reaction("👍");
                return Ok(resp)
            }
            Some("admin") => {
                command.require_admin().await?;
                return Ok("x".into())
            }
            Some("big_msg") => {
                command.require_admin().await?;
                
                let mut text: String = "".into();
                for _ in 0 .. 500 {
                    text.push_str("bwoooo ");
                }
                return Ok(text.into())
            }
            Some("members") => {
                command.require_global_admin().await?;
                
                let guild_id = command.message.guild_id.clone().unwrap();
                let members = command.send_handle.get_guild_members(guild_id).await.unwrap();
                
                let mut text: String = "".into();
                for member in members {
                    if let Some(user) = member.user {
                        text.push_str(&user.username);
                        text.push_str("\n");
                    }
                }
                return Ok(text.into())
            }
            Some(_) => (),
            None => {
                
                
                // let is_admin = is_msg_from_admin(&command.message).unwrap();
                let is_admin = false;
                
                let text = format!("admin {:?}", is_admin);
                
                return Ok(text.into())
            }
        }
        
        Ok(().into())
    }
}
