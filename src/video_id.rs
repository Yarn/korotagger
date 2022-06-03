
use crate::i_love_youtube::extract_id;
use crate::bili::extract_id as bili_id;
use crate::twitch::extract_id as twitch_id;

#[derive(Debug)]
pub enum VideoId {
    Youtube(String),
    Twitch(String),
    Bili(String),
    Other(String),
}

impl VideoId {
    pub fn extract(stream_name: &str) -> Self {
        if let Some(id) = extract_id(stream_name) {
            VideoId::Youtube(id.to_owned())
        } else if let Some(id) = twitch_id(stream_name) {
            VideoId::Twitch(id)
        } else if let Some(id) = bili_id(stream_name) {
            VideoId::Bili(id.to_owned())
        } else {
            VideoId::Other(stream_name.to_owned())
        }
    }
    
    pub fn format_discord_offset(&self, offset: &str) -> String {
        match self {
            VideoId::Youtube(id) => {
                format!(
                    "[{offset}](https://youtu.be/{id}?t={offset})",
                    offset = offset,
                    id = id,
                )
            }
            VideoId::Twitch(id) => {
                format!(
                    "[{offset}](https://twitch.tv/videos/{id}?t={offset})",
                    offset = offset,
                    id = id,
                )
            }
            VideoId::Bili(id) => {
                format!(
                    "[{offset}](https://www.bilibili.com/video/{id}?t={offset})",
                    offset = offset,
                    id = id,
                )
            }
            VideoId::Other(_stream_name) => {
                format!("({})", offset)
            }
        }
    }
}
