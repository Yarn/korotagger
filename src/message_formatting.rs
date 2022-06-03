
use std::borrow::Cow;
use discord_lib::send_message::NewMessage;

// \ needs to be escaped first or it will escape the escaping \s
const ESCAPED_CHARS: &[char] = &[ '\\',   '~',   '`',   '*',   '_',   '|'];
const ESCAPED: &[&str] =       &["\\\\","\\~", "\\`", "\\*", "\\_", "\\|"];

const EMBED_CHARS: &[char] =   &[  '[',   ']',   '(',   ')'];
const EMBED_ESCAPED: &[&str] = &["\\[", "\\]", "\\(", "\\)"];

#[allow(dead_code)]
pub fn escape_text(text: &str) -> Cow<str> {
    if text.contains(ESCAPED_CHARS) {
        let mut text = text.to_string();
        for (c, s) in ESCAPED_CHARS.iter().zip(ESCAPED.iter()) {
            text = text.replace(*c, s);
        }
        Cow::Owned(text)
    } else {
        Cow::Borrowed(text)
    }
}

pub fn escape_embed(text: &str) -> Cow<str> {
    if text.contains(ESCAPED_CHARS) || text.contains(EMBED_CHARS) {
        let mut text = text.to_string();
        for (c, s) in ESCAPED_CHARS.iter().zip(ESCAPED.iter()) {
            text = text.replace(*c, s);
        }
        for (c, s) in EMBED_CHARS.iter().zip(EMBED_ESCAPED.iter()) {
            text = text.replace(*c, s);
        }
        Cow::Owned(text)
    } else {
        Cow::Borrowed(text)
    }
}

fn split_msg(msg: &str, max_len: usize) -> Vec<Cow<str>> {
    if msg.len() > max_len {
        let mut new_out = vec![Cow::Owned(String::new())];
        let split = msg.lines();
        
        let mut last_line_long = false;
        for line in split {
            if last_line_long {
                last_line_long = false;
                new_out.push("".into());
            }
            
            if line.len() > max_len {
                last_line_long = true;
                if new_out.last().unwrap() != "" {
                    new_out.push("".into());
                }
                
                for c in line.chars() {
                    if new_out.last().unwrap().len() >= max_len {
                        new_out.push("".into());
                    }
                    
                    new_out.last_mut().unwrap().to_mut().push(c);
                }
                
                continue
            }
            
            // add 1 for \n added if false
            if new_out.last().unwrap().len() + line.len() + 1 > max_len {
                new_out.push(Cow::Owned("".into()));
            } else {
                new_out.last_mut().unwrap().to_mut().push('\n');
            }
            new_out.last_mut().unwrap().to_mut().push_str(line);
        }
        
        for msg in new_out.iter_mut() {
            if msg.trim() == "" {
                msg.to_mut().push('.')
            }
        }
        
        new_out
    } else {
        vec![Cow::Borrowed(msg)]
    }
}

pub fn wrapped_desc_message<'a>(title: Option<&'a str>, text: &'a str) -> Vec<NewMessage<'a>> {
    let mut out = Vec::new();
    
    let mut msgs = split_msg(text, 2000).into_iter();
    
    if let Some(msg) = msgs.next() {
        let msg: Cow<str> = msg;
        let new_msg = NewMessage::embed_desc(title, msg);
        out.push(new_msg);
    }
    for msg in msgs {
        let new_msg = NewMessage::embed_desc::<&str, _>(None, msg);
        out.push(new_msg);
    }
    
    out
}
