
use sqlx::PgPool;
use async_trait::async_trait;
use discord_lib::discord::Message;
use discord_lib::SendHandle;
use discord_lib::send_message::NewMessage;
use discord_lib::gateway::{ MessageReactionAdd, MessageReactionRemove };

pub mod tagging;
// pub mod histogram;
pub mod test_handler;
pub mod streams;
pub mod config;
pub mod jank;

#[derive(Debug)]
pub struct HandlerError {
    pub error_message: String,
    _priv: (),
}

impl HandlerError {
    pub fn with_message(msg: String) -> Self {
        HandlerError {
            error_message: msg,
            _priv: (),
        }
    }
}

// impl<T: std::error::Error> From<T> for HandlerError {
//     fn from(err: T) -> Self {
//         HandlerError {
//             error_message: format!("{}", err),
//             _priv: (),
//         }
//     }
// }

// impl From<&'_ str> for HandlerError {
//     fn from(msg: &str) -> Self {
//         HandlerError {
//             error_message: msg.into(),
//             _priv: (),
//         }
//     }
// }

#[derive(Debug)]
pub struct HandlerResponse {
    pub messages: Option<Vec<NewMessage<'static>>>,
    pub reactions: Option<Vec<String>>,
}

impl HandlerResponse {
    pub fn wrapped_embed(title: Option<&str>, msg: &str) -> Self {
        let msgs: Vec<_> = crate::message_formatting::
            wrapped_desc_message(title, msg)
            .into_iter()
            .map(|x| x.into_owned())
            .collect();
        
        HandlerResponse {
            messages: Some(msgs),
            reactions: Some(Vec::new()),
        }
    }
    
    pub fn add_reaction(&mut self, emoji: &str) {
        let reactions = self.reactions.get_or_insert_with(|| Vec::new());
        
        reactions.push(emoji.to_owned());
    }
}

impl From<String> for HandlerResponse {
    fn from(msg: String) -> Self {
        HandlerResponse {
            messages: Some(vec![msg.into()]),
            reactions: Some(Vec::new()),
        }
    }
}

impl From<()> for HandlerResponse {
    fn from(_: ()) -> Self {
        HandlerResponse {
            messages: None,
            reactions: Some(Vec::new()),
        }
    }
}

impl<'a> From<&'a str> for HandlerResponse {
    fn from(msg: &str) -> Self {
        let msg: String = msg.into();
        HandlerResponse {
            messages: Some(vec![msg.into()]),
            reactions: Some(Vec::new()),
        }
    }
}

impl<'a> From<NewMessage<'a>> for HandlerResponse {
    fn from(msg: NewMessage<'a>) -> Self {
        HandlerResponse {
            messages: Some(vec![msg.into_owned()]),
            reactions: Some(Vec::new()),
        }
    }
}

impl<'a> From<Vec<NewMessage<'a>>> for HandlerResponse {
    fn from(msgs: Vec<NewMessage<'a>>) -> Self {
        let msgs: Vec<_> = msgs.into_iter().map(|x| x.into_owned()).collect();
        HandlerResponse {
            messages: Some(msgs),
            reactions: Some(Vec::new()),
        }
    }
}
// impl<'a, T: Iterator<Item = NewMessage<'a>>> From<T> for HandlerResponse {
//     fn from(msgs: T) -> Self {
//         let msgs: Vec<_> = msgs.map(|x| x.into_owned()).collect();
//         HandlerResponse {
//             messages: Some(msgs),
//         }
//     }
// }

// #[macro_export]
// macro_rules! error {
//     ($e: expr,+) => {
//         HandlerError {
//             error_message: format!($e),
//             _priv: ()
//         }
//     }
// }

#[derive(Debug)]
pub struct SimpleHelpInfo {
    pub arg_str: String,
    pub desc: String,
}

impl<'a> From<(&'a str, &'a str)> for SimpleHelpInfo {
    fn from(tup: (&str, &str)) -> Self {
        Self {
            arg_str: tup.0.into(),
            desc: tup.1.into(),
        }
    }
}

use crate::permissions::{
    is_msg_from_admin,
    is_msg_from_global_admin,
};

pub type HandlerResult = Result<HandlerResponse, HandlerError>;

pub struct Command<'a> {
    pub args: &'a [&'a str],
    pub message: &'a Message,
    pub send_handle: &'a SendHandle,
    pub state: &'a crate::DiscordState,
    pub pool: &'a PgPool,
}

impl Command<'_> {
    pub async fn require_admin(&self) -> Result<(), HandlerError> {
        let is_admin = is_msg_from_admin(self.pool, self.message).await
            .map_err(|e| HandlerError::with_message(format!("admin check failed {:?}", e)))?;
        
        if is_admin {
            Ok(())
        } else {
            Err(HandlerError::with_message("permission oof".to_string()))
        }
    }
    
    pub async fn require_global_admin(&self) -> Result<(), HandlerError> {
        let is_admin = is_msg_from_global_admin(self.pool, self.message).await
            .map_err(|e| HandlerError::with_message(format!("global admin check failed {:?}", e)))?;
        
        if is_admin {
            Ok(())
        } else {
            Err(HandlerError::with_message("big permission oof".to_string()))
        }
    }
}

#[async_trait]
pub trait Handler: Sync + Send + std::fmt::Debug {
    async fn handle_command(&self, _args: &[&str], _msg: &Message) -> HandlerResult {
        Ok(().into())
    }
    
    async fn handle_command_b(&self, _command: Command<'_>) -> HandlerResult {
        Ok(().into())
    }
    
    async fn handle_message(&self, _msg: &Message) -> HandlerResult {
        Ok(().into())
    }
    
    async fn handle_message_b(&self, _msg: Command<'_>) -> HandlerResult {
        Ok(().into())
    }
    
    async fn handle_reaction_add_simple(&self, _reaction: &MessageReactionAdd) {}
    
    async fn handle_reaction_remove_simple(&self, _reaction: &MessageReactionRemove) {}
    
    fn help_info_simple(&self) -> Option<SimpleHelpInfo> {
        None
    }
}


