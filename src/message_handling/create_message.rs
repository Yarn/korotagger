
use discord_lib::discord::Message;
use discord_lib::SendHandle;
use std::sync::Arc;
use std::collections::BTreeMap;
use std::panic::AssertUnwindSafe;
use futures::prelude::*;

use crate::DiscordState;
use crate::command_parsing;

pub async fn handle_message_create(
    state: Arc<DiscordState>,
    msg: Message,
) -> Result<(), ()> {
    
    let ref send_handle = state.send_handle;
    let ref pool = state.pool;
    let handlers = state.handlers.as_ref();
    let command_aliases = state.command_aliases.as_ref();
    
    let ref content = msg.content;
    
    #[inline(always)]
    fn extract_command(content: &str, command_aliases: &BTreeMap<&str, &str>) -> Result<Option<(String, Vec<String>)>, String> {
        if !content.starts_with("!") {
            return Ok(None)
        }
        if content.starts_with("!tag ") || content.starts_with("!t ") {
            return Ok(None)
        }
        
        let (command, args) = match command_parsing::parse_command(content) {
            Ok(res) => res,
            Err(err) => {
                // dbg!(&err);
                let err_text = format!("{}", err);
                let err_text = err_text.replace("`", "\u{200D}`");
                let err_text = format!("```{}```", err_text);
                
                return Err(err_text);
            }
        };
        
        let mut command = command[1..].to_string();
        command.make_ascii_lowercase();
        
        if let Some(new_command) = command_aliases.get(command.as_str()) {
            command = new_command.to_string()
        }
        
        let args: Vec<String> = args.iter().map(|x| x.to_string()).collect();
        
        let command = command.to_string();
        Ok(Some((command, args)))
    }
    
    let command = match extract_command(content, &*command_aliases) {
        Ok(command) => command,
        Err(err_text) => {
            let send_msg: discord_lib::send_message::NewMessage = err_text.into();
            
            let to = msg.channel_id.clone();
            let fut = send_handle.send(to, &send_msg);
            if let Err(err) = fut.await {
                dbg!("Failed to send message: {:?}", err);
            }
            None
        }
    };
    
    use crate::handlers::HandlerResponse;
    async fn send_response(msg: &Message, res: &HandlerResponse, send_handle: &SendHandle) {
        if let Some(ref msgs) = res.messages {
            for send_msg in msgs {
                let to = msg.channel_id.clone();
                
                let fut = send_handle.send(to, &send_msg);
                if let Err(err) = fut.await {
                    dbg!("Failed to send message: {:?}", err);
                }
                // delay_for(::std::time::Duration::from_millis(500)).await;
            }
        }
        if let Some(ref reactions) = res.reactions {
            for reaction in reactions {
                let fut = send_handle.set_reaction(
                    msg.channel_id.clone(),
                    msg.id.clone(),
                    &reaction,
                );
                if let Err(err) = fut.await {
                    dbg!("Failed to set reaction: {:?}", err);
                }
            }
        }
    }
    
    for handler in state.handlers.values() {
        match AssertUnwindSafe(handler.handle_message(&msg)).catch_unwind().await {
            Ok(Ok(res)) => {
                send_response(&msg, &res, &send_handle).await;
            }
            Ok(Err(err)) => {
                dbg!(&err);
                
                let send_msg: discord_lib::send_message::NewMessage = err.error_message.into();
                
                let to = msg.channel_id.clone();
                let fut = send_handle.send(to, &send_msg);
                if let Err(err) = fut.await {
                    dbg!("Failed to send message: {:?}", err);
                }
            }
            Err(err) => {
                dbg!(err);
            }
        }
        
        static PLACEHOLDER_ARGS: Vec::<&'_ str> = Vec::new();
        use crate::handlers::Command;
        let command = Command {
            args: &PLACEHOLDER_ARGS,
            message: &msg,
            pool: &pool,
            send_handle: &send_handle,
            state: &state,
        };
        
        match AssertUnwindSafe(handler.handle_message_b(command)).catch_unwind().await {
            Ok(Ok(res)) => {
                send_response(&msg, &res, &send_handle).await;
            }
            Ok(Err(err)) => {
                dbg!(handler, &err);
                let send_msg: discord_lib::send_message::NewMessage = err.error_message.into();
                
                let to = msg.channel_id.clone();
                let fut = send_handle.send(to, &send_msg);
                if let Err(err) = fut.await {
                    dbg!("Failed to send message: {:?}", err);
                }
            }
            Err(err) => {
                dbg!(handler, err);
                
                let send_msg: discord_lib::send_message::NewMessage = "Something went very wrong".into();
                
                let to = msg.channel_id.clone();
                let fut = send_handle.send(to, &send_msg);
                if let Err(err) = fut.await {
                    dbg!("Failed to send message: {:?}", err);
                }
            }
        }
    }
    
    if let Some((command, args)) = command {
        let command: &str = &command;
        let args: Vec<&str> = args.iter().map(|x| x.as_str()).collect();
        
        if let Some(handler) = handlers.get(command) {
            match AssertUnwindSafe(handler.handle_command(&args, &msg)).catch_unwind().await {
                Ok(Ok(res)) => {
                    send_response(&msg, &res, &send_handle).await;
                }
                Ok(Err(err)) => {
                    dbg!(command, &err);
                    // reply!(err.error_message);
                    let send_msg: discord_lib::send_message::NewMessage = err.error_message.into();
                    
                    let to = msg.channel_id.clone();
                    let fut = send_handle.send(to, &send_msg);
                    if let Err(err) = fut.await {
                        dbg!("Failed to send message: {:?}", err);
                    }
                }
                Err(err) => {
                    dbg!(err);
                    
                    let send_msg: discord_lib::send_message::NewMessage = "Something went very wrong".into();
                    
                    let to = msg.channel_id.clone();
                    let fut = send_handle.send(to, &send_msg);
                    if let Err(err) = fut.await {
                        dbg!("Failed to send message: {:?}", err);
                        // continue 'msg_recv
                        // break
                    }
                }
            }
            
            use crate::handlers::Command;
            let command = Command {
                args: &args,
                message: &msg,
                pool: &pool,
                send_handle: &send_handle,
                state: &state,
            };
            
            match AssertUnwindSafe(handler.handle_command_b(command)).catch_unwind().await {
                Ok(Ok(res)) => {
                    send_response(&msg, &res, &send_handle).await;
                }
                Ok(Err(err)) => {
                    dbg!(handler, &err);
                    // reply!(err.error_message);
                    let send_msg: discord_lib::send_message::NewMessage = err.error_message.into();
                    
                    let to = msg.channel_id.clone();
                    let fut = send_handle.send(to, &send_msg);
                    if let Err(err) = fut.await {
                        dbg!("Failed to send message: {:?}", err);
                    }
                }
                Err(err) => {
                    dbg!(handler, err);
                    
                    let send_msg: discord_lib::send_message::NewMessage = "Something went very wrong".into();
                    
                    let to = msg.channel_id.clone();
                    let fut = send_handle.send(to, &send_msg);
                    if let Err(err) = fut.await {
                        dbg!("Failed to send message: {:?}", err);
                    }
                }
            }
            
        }
    }
    Ok(())
}

