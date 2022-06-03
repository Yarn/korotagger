
// use async_trait::async_trait;
// use std::sync::Arc;
use std::collections::BTreeMap;
use crate::handlers::Handler;
// use crate::Message;
// use crate::handlers::HandlerResult;

pub fn get_help_text<'a>(
    handlers: &BTreeMap<&'a str, Box<dyn Handler>>,
    aliases: &BTreeMap<&'a str, &'a str>,
    prefix: &'a str,
    ) -> String
{
    
    let mut out: String = "".into();
    
    for (command, handler) in handlers.iter() {
        if let Some(help_info) = handler.help_info_simple() {
            out.push_str(prefix);
            out.push_str(command);
            for (alias, map_to) in aliases {
                if map_to == command {
                    out.push_str(&format!(", {}{}", prefix, alias));
                }
            }
            out.push('\n');
            out.push_str(&format!("`{}` {}", help_info.arg_str, help_info.desc));
            out.push('\n');
        }
    }
    
    out
}

// #[derive(Debug)]
// pub struct HelpHandler {
//     handlers: Arc<BTreeMap<&'static str, Box<dyn Handler>>>,
//     aliases: BTreeMap<&'static str, &'static str>,
//     prefix: &'static str,
// }

// #[async_trait]
// impl Handler for HelpHandler {
//     async fn handle_command(&self, args: &[&str], msg: &Message) -> HandlerResult {
//         Ok(().into())
//     }
// }
