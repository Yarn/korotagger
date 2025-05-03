
use discord_lib::discord;
use rustbreak::FileDatabase;
use rustbreak::deser::Ron;
#[allow(unused_imports)]
use std::collections::{ HashMap, HashSet, BTreeMap, BTreeSet };
// use std::time::SystemTime;
use serde::{Serialize, Deserialize};
use chrono::{ DateTime, FixedOffset };
use tokio::time::sleep;
use tokio;
use futures::FutureExt as _;
use std::panic::AssertUnwindSafe;
use tokio::runtime::Runtime;

mod auto_stream_live;
mod i_love_youtube;
mod permissions;
mod message_formatting;
mod command_parsing;
mod handlers;
mod help_text;
mod bili;
mod twitch;
mod video_id;
mod db_util;
mod util;
mod server_settings;
mod auto_live;

use futures::lock::Mutex;
use std::sync::Arc;
use discord_lib::discord::Snowflake;
use discord_lib::Channel;
use discord_lib::SendHandle;

use sqlx::PgPool;

type DateTimeF = DateTime<FixedOffset>;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SharedState {
    #[serde(default)]
    active: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    session_id: Option<String>,
    seq: Option<u64>,
    self_id: Option<u64>,
    #[serde(default)]
    guilds: HashSet<u64>,
}

impl SessionState {
    fn new() -> Self {
        SessionState {
            session_id: None,
            seq: None,
            self_id: None,
            guilds: HashSet::new(),
        }
    }
}

use discord_lib::discord::Message;
fn get_mention_ids(msg: &Message) -> Result<Vec<u64>, &str> {
    let ids = if msg.mentions.is_empty() && msg.mention_roles.is_empty() {
        let new_id: &str = msg.content.splitn(3, " ").skip(2).next()
            .ok_or("Incorrect parameters")?;
        
        let new_id: u64 = match new_id.parse() {
            Ok(id) => id,
            Err(_) => {
                return Err("Invalid id format");
            }
        };
        
        vec![new_id]
    } else {
        let mut ids = Vec::new();
        
        for mention in msg.mentions.iter() {
            ids.push(mention.id.0);
        }
        
        for role in msg.mention_roles.iter() {
            ids.push(role.0);
        }
        
        ids
    };
    
    Ok(ids)
}

#[derive(Debug)]
enum GetPoolError {
    MissingEnv,
    #[allow(dead_code)]
    Sqlx(sqlx::Error),
}

async fn get_pool() -> Result<PgPool, GetPoolError> {
    let pg_url = match std::env::var("pg_url") {
        Ok(token) => token,
        Err(_e) => {
            return Err(GetPoolError::MissingEnv);
        }
    };
    
    use sqlx::postgres::PgPoolOptions;
    
    let pool: PgPool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&pg_url).await.map_err(|e| GetPoolError::Sqlx(e))?;
    
    Ok(pool)
}

#[derive(Debug, Clone)]
pub struct DiscordState {
    // list of guilds the bot is in
    name: Arc<String>,
    servers: Arc<Mutex<BTreeSet<Snowflake>>>,
    channel_cache: Arc<Mutex<BTreeMap<u64, Channel>>>,
    send_handle: SendHandle,
    session: Arc<FileDatabase<SessionState, Ron>>,
    #[allow(dead_code)]
    shared: Arc<FileDatabase<SharedState, Ron>>,
}

impl DiscordState {
    pub fn new(
        name: String,
        send_handle: SendHandle,
        session: Arc<FileDatabase<SessionState, Ron>>,
        shared: Arc<FileDatabase<SharedState, Ron>>,
    ) -> Self {
        Self {
            name: Arc::new(name),
            servers: Arc::new(Mutex::new(BTreeSet::new())),
            channel_cache: Arc::new(Mutex::new(BTreeMap::new())),
            send_handle: send_handle,
            session,
            shared,
        }
    }
    
    pub async fn new_session(&self) {
        self.servers.lock().await.clear();
    }
    
    pub async fn get_channel(&self, channel_id: Snowflake) -> Result<Channel, anyhow::Error> {
        let channel_id = channel_id.0;
        
        let get_res = {
            let channel_cache = self.channel_cache.lock().await;
            channel_cache.get(&channel_id).map(|x| x.clone())
        };
        
        let send_handle = &self.send_handle;
        
        let channel = match get_res {
            Some(channel) => channel,
            None => {
                let channel_sf = Snowflake(channel_id);
                let channel = match send_handle.get_channel(channel_sf).await {
                    Ok(channel) => channel,
                    Err(err) => {
                        println!("channel failed {} {:?}", channel_id, err);
                        return Err(anyhow::anyhow!("channel failed {} {:?}", channel_id, err))
                    }
                };
                // println!("{:#?}", channel);
                {
                    let mut channel_cache = self.channel_cache.lock().await;
                    let channel = channel_cache.entry(channel_id).or_insert(channel);
                    channel.clone()
                }
            }
        };
        
        Ok(channel)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct BotConfig {
    name: String,
    token: String,
}

#[derive(Debug)]
pub struct Bot {
    discord: discord_lib::Discord,
    state: DiscordState,
    seq: Option<u64>,
}

impl Bot {
    async fn new(
        shared_state: Arc<FileDatabase<SharedState, Ron>>,
        session_state: Arc<FileDatabase<SessionState, Ron>>,
        name: &str,
        token: &str,
    ) -> Self {
        
        
        let mut last_seq: Option<u64> = None;
        let discord_obj = {
            let mut data = session_state.borrow_data_mut().unwrap();
            match (&data.session_id, &data.seq) {
                (Some(ref session_id), Some(seq)) => {
                    last_seq = data.seq;
                    
                    discord_lib::Discord::reconnect_with(BASE_URL.into(), token.to_string(), session_id.clone(), *seq).await.unwrap()
                }
                _ => {
                    data.session_id = None;
                    data.seq = None;
                    discord_lib::Discord::connect(BASE_URL.into(), token.to_string()).await.unwrap()
                }
            }
        };
        
        let send_handle = discord_obj.get_send_handle();
        let d_state = DiscordState::new(
            name.to_string(),
            send_handle.clone(),
            session_state.clone(),
            shared_state.clone(),
        );
        
        let bot = Bot {
            discord: discord_obj,
            state: d_state,
            seq: last_seq,
        };
        
        bot
    }
}

const BASE_URL: &'static str = "https://discord.com/api/v9";

async fn discord_stuff() {
    let bot_config = match std::env::var("bots") {
        Ok(raw) => {
            let config: Vec<BotConfig> = serde_json::from_str(&raw)
                .expect("failed to parse bots environment variable");
            config
        }
        Err(e) => {
            eprintln!("Failed to get env var bots: {:?}", e);
            return;
        }
    };
    
    let state_path = match std::env::var("state_path") {
        Ok(s) => {
            let path: std::path::PathBuf = s.into();
            path
        }
        Err(_) => {
            let mut path = std::path::PathBuf::new();
            path.push(".");
            path
        }
    };
    eprintln!("state path: {:?}", state_path);
    match std::fs::create_dir(&state_path) {
        Ok(_) => (),
        Err(err) => {
            match err.kind() {
                std::io::ErrorKind::AlreadyExists => (),
                _ => panic!("could not create state path {:?}", err)
            }
        }
    }
    
    // load to throw error early if environment variable isn't set
    let _ = &*i_love_youtube::YT_API_KEY;
    let holodex_api_key = std::env::var("holodex_api_key").expect("environment variable holodex_api_key not set");
    
    let pool = match get_pool().await {
        Ok(pool) => pool,
        Err(GetPoolError::MissingEnv) => {
            eprintln!("Failed to get env var pg_url");
            return;
        }
        Err(e) => panic!("{:?}", e),
    };
    
    let mut path = state_path.clone();
    path.push("kq_shared.ron");
    let shared_state: FileDatabase<SharedState, Ron> = FileDatabase::from_path(&path, SharedState::default()).unwrap();
    let _ = shared_state.load();
    let shared_state = Arc::new(shared_state);
    
    let mut bots: Vec<Bot> = Vec::new();
    for config in bot_config {
        let mut path = state_path.clone();
        path.push(&format!("kq_session_{}.ron", &config.name));
        let session_state = FileDatabase::from_path(
            &path,
            SessionState::new(),
        ).unwrap();
        let _ = session_state.load();
        let session_state = Arc::new(session_state);
        
        let bot = Bot::new(
            shared_state.clone(),
            session_state,
            &config.name,
            &config.token,
        ).await;
        eprintln!("{:>10}: seq: {:?}", bot.state.name, bot.seq);
        bots.push(bot);
    }
    
    let task_pool = pool.clone();
    let task_shared_state = shared_state.clone();
    let states: Vec<DiscordState> = bots.iter().map(|b| b.state.clone()).collect();
    
    let _task_handle = tokio::task::spawn(async move {
        let shared_state = task_shared_state;
        let mut active = {
            let data = shared_state.borrow_data().unwrap();
            data.active.clone()
        };
        
        let mut channel_cache: auto_stream_live::ChannelCache = HashMap::new();
        loop {
            let task = auto_live::holodex::auto_live_task(
                &mut active, &task_pool,
                states.as_slice(),
                &mut channel_cache,
                &holodex_api_key,
            );
            
            match task.await {
                Ok(()) => {
                    {
                        let mut data = shared_state.borrow_data_mut().unwrap();
                        
                        data.active = active.clone();
                    }
                    shared_state.save().unwrap();
                },
                Err(err) => {
                    eprintln!("auto live task failed {:?}", err);
                }
            }
            
            // delay_for(::std::time::Duration::new(30, 0)).await;
            sleep(::std::time::Duration::new(30, 0)).await;
        }
    });
    
    use handlers::{
        Handler, HandlerResult,
    };
    
    #[derive(Debug)]
    struct XPotatoHandler {
        
    }
    
    use async_trait::async_trait;
    #[async_trait]
    impl Handler for XPotatoHandler {
        async fn handle_command(&self, _args: &[&str], _msg: &Message) -> HandlerResult {
            println!("xpotato");
            panic!();
            // Ok("xpotato".into())
        }
    }
    
    let mut handlers:
        BTreeMap<&'static str, Box<dyn Handler>>
        = BTreeMap::new();
    
    use handlers::tagging::{ TagsHandler, TagHandler, AdjustHandler };
    // use handlers::histogram::HistogramHandler;
    use handlers::test_handler::TestHandler;
    use handlers::streams::{ OffsetHandler, YtStartHandler, TwitchStartHandler, TwitterSpaceStartHandler, SetStreamHandler, ManualStartHandler };
    use handlers::config::{ SubscribeHandler, ManageAdminHandler };
    use handlers::jank::CopyTagsHandler;
    // use members::handlers::{ MembersHandler, MembersAdminHandler, VerifyHandler };
    use auto_live::channel_watch::ChannelWatchHandler;
    // use help_text::HelpHandler;
    
    handlers.insert("xpotato", Box::new(XPotatoHandler {}));
    handlers.insert("tags", Box::new(TagsHandler { pool: pool.clone() }));
    handlers.insert("tag", Box::new(TagHandler { vote_emoji: "⭐".into(), delete_emoji: "❌".into(), pool: pool.clone() }));
    handlers.insert("test", Box::new(TestHandler {}));
    handlers.insert("offset", Box::new(OffsetHandler { pool: pool.clone() }));
    handlers.insert("adjust", Box::new(AdjustHandler { pool: pool.clone() }));
    handlers.insert("copy_tags", Box::new(CopyTagsHandler { pool: pool.clone() }));
    handlers.insert("set_start", Box::new(ManualStartHandler { pool: pool.clone() }));
    handlers.insert("yt_start", Box::new(YtStartHandler { pool: pool.clone() }));
    handlers.insert("twitch_start", Box::new(TwitchStartHandler { pool: pool.clone() }));
    handlers.insert("spaces_start", Box::new(TwitterSpaceStartHandler { pool: pool.clone() }));
    // handlers.insert("streams", Box::new(ListStreamsHandler { pool: pool.clone() }));
    handlers.insert("stream", Box::new(SetStreamHandler { pool: pool.clone() }));
    handlers.insert("sub", Box::new(SubscribeHandler { pool: pool.clone() }));
    handlers.insert("admin", Box::new(ManageAdminHandler { pool: pool.clone() }));
    
    handlers.insert("watch_channel", Box::new(ChannelWatchHandler::new(pool.clone())));
    
    let mut command_aliases = BTreeMap::new();
    command_aliases.insert("t", "tag");
    
    let handlers = Arc::new(handlers);
    
    struct MessageWrapper {
        msg: discord_lib::gateway::GatewayMessage,
        self_id: Snowflake,
        state: Arc<DiscordState>,
    }
    
    use tokio::sync::mpsc;
    async fn handle_discord_connection(
        sender: mpsc::Sender<MessageWrapper>,
        mut bot: Bot,
    ) {
        let mut self_id = {
            let data = bot.state.session.borrow_data_mut().unwrap();
            data.self_id.map(|x| discord::Snowflake(x))
        };
        let ref mut discord_obj = bot.discord;
        let ref discord_state = bot.state;
        let _name = discord_state.name.as_str();
        let ref session_state = discord_state.session;
        
        let shared_state = Arc::new(discord_state.clone());
        
        let mut last_seq = bot.seq;
        
        loop {
            use discord_lib::gateway::GatewayError;
            match discord_obj.recv().await {
                Ok(msg) => {
                    use discord_lib::gateway::GatewayMessage as GM;
                    use discord_lib::gateway::Event as E;
                    
                    match &msg {
                        GM::Event(E::Ready(ready)) => {
                            {
                                let mut data = session_state.borrow_data_mut().unwrap();
                                data.session_id = Some(ready.session_id.clone());
                                data.self_id = Some(ready.user.id.0);
                            }
                            session_state.save().unwrap();
                            self_id = Some(ready.user.id);
                            let current_guilds = {
                                let mut servers = discord_state.servers.lock().await;
                                let servers = &mut *servers;
                                for guild in ready.guilds.iter() {
                                    servers.insert(guild.id.clone());
                                }
                                servers.iter().map(|x| x.0).collect()
                            };
                            // dbg!(&current_guilds);
                            {
                                let mut data = session_state.borrow_data_mut().unwrap();
                                data.guilds = current_guilds;
                            }
                            session_state.save().unwrap();
                        }
                        GM::Event(E::Unknown(event_type, event_data)) if *event_type == "GUILD_CREATE".to_string() => {
                            // eprintln!("{:#?}", event_data);
                            let event_data = event_data.as_object().unwrap();
                            // owner_id
                            // id
                            let guild_id: u64 = event_data.get("id").unwrap().as_str().unwrap().parse().unwrap();
                            // let owner_id: u64 = event_data.get("owner_id").unwrap().as_str().unwrap().parse().unwrap();
                            
                            let current_guilds = {
                                let mut servers = discord_state.servers.lock().await;
                                let servers = &mut *servers;
                                servers.insert(Snowflake(guild_id));
                                servers.iter().map(|x| x.0).collect()
                            };
                            // dbg!(&current_guilds);
                            {
                                let mut data = session_state.borrow_data_mut().unwrap();
                                data.guilds = current_guilds;
                            }
                            session_state.save().unwrap();
                        }
                        GM::Event(E::Unknown(event_type, event_data)) if *event_type == "GUILD_DELETE".to_string() => {
                            let event_data = event_data.as_object().unwrap();
                            
                            let guild_id: u64 = event_data.get("id").unwrap().as_str().unwrap().parse().unwrap();
                            
                            let current_guilds = {
                                let mut servers = discord_state.servers.lock().await;
                                let servers = &mut *servers;
                                servers.remove(&Snowflake(guild_id));
                                servers.iter().map(|x| x.0).collect()
                            };
                            dbg!(&current_guilds);
                            {
                                let mut data = session_state.borrow_data_mut().unwrap();
                                data.guilds = current_guilds;
                            }
                            session_state.save().unwrap();
                        }
                        GM::Event(E::Unknown(event_type, _)) if *event_type == "RESUMED".to_string() => {
                            let guilds = {
                                let data = session_state.borrow_data().unwrap();
                                data.guilds.iter().map(|x| Snowflake(*x)).collect()
                            };
                            let ref mut servers = discord_state.servers.lock().await;
                            let servers = &mut **servers;
                            *servers = guilds;
                        }
                        _ => {}
                    }
                    
                    if let Some(self_id) = self_id {
                        let out = MessageWrapper {
                            msg: msg,
                            self_id: self_id,
                            state: Arc::clone(&shared_state),
                        };
                        sender.send(out).await.unwrap();
                    }
                },
                Err(err) => {
                    eprintln!("Err recieving message: {:?}", err);
                    
                    if let GatewayError::InvalidSession = err {
                        eprintln!("InvalidSession: ");
                        discord_state.new_session().await;
                        last_seq = None;
                        {
                            let mut data = session_state.borrow_data_mut().unwrap();
                            
                            data.session_id = None;
                            data.seq = None;
                        }
                        // db.async_save_session().await.unwrap();
                        session_state.save().unwrap();
                    }
                    
                    let mut wait_secs = 1;
                    loop {
                        // match discord_lib::Discord::connect(BASE_URL.into(), token.clone()).await {
                        match discord_obj.reconnect().await {
                            Ok(is_reconnect) => {
                                if is_reconnect {
                                    eprintln!("IS RECONNECT");
                                    // reconnecting = true;
                                }
                                break
                            }
                            Err(err) => {
                                println!("Could not re-establish connection ({}s): {:?}", wait_secs, err);
                                // use discord_lib::tokio::time::delay_for;
                                sleep(::std::time::Duration::new(wait_secs, 0)).await;
                                if wait_secs < 20 {
                                    wait_secs *= 2;
                                }
                            }
                        }
                    };
                    continue
                },
            }
            // dbg!(&msg);
            
            let new_seq = discord_obj.seq();
            if new_seq != last_seq {
                last_seq = new_seq;
                {
                    let mut data = session_state.borrow_data_mut().unwrap();
                    data.seq = new_seq;
                }
                session_state.save().unwrap();
            }
        }
    }
    
    let (send, mut recv) = mpsc::channel(200);
    for bot in bots.into_iter() {
        tokio::task::spawn(handle_discord_connection(
            send.clone(),
            bot,
        ));
    }
    
    static PLACEHOLDER_ARGS: Vec::<&'_ str> = Vec::new();
    
    loop {
        // dbg!("loop start");
        let wrapper = recv.recv().await.unwrap();
        let msg = wrapper.msg;
        let send_handle = wrapper.state.send_handle.clone();
        let self_id = Some(wrapper.self_id);
        let name = wrapper.state.name.as_str();
        
        use discord_lib::gateway::GatewayMessage as GM;
        use discord_lib::gateway::Event as E;
        
        match msg {
            GM::Event(E::MessageCreate(msg)) => {
                // let ref incoming_msg = msg;
                // let reply_macro_to = msg.channel_id.clone();
                
                // ignore messages from self
                if let Some(ref self_id) = self_id {
                    if &msg.author.id == self_id {
                        continue;
                    }
                }
                
                // println!("-------\n\nMESSAGE CREATE");
                // println!("{:#?}\n------------\n\n", msg);
                let ref content = msg.content;
                
                // let (command, args) = command_parsing::parse_command(content).unwrap();
                // dbg!(&command, &args);
                
                if false {
                
                } else if content == "!help" {
                    let text = help_text::get_help_text(&*handlers, &command_aliases, "!");
                    
                    let send_msg: discord_lib::send_message::NewMessage = text.into();
                    
                    let to = msg.channel_id.clone();
                    let fut = send_handle.send(to, &send_msg);
                    if let Err(err) = fut.await {
                        dbg!("Failed to send message: {:?}", err);
                        continue
                    }
                } else if content == "!voice" {
                    // let msg = r#"
                    //     {
                    //         "op": 4,
                    //         "d": {
                    //             "guild_id": "-",
                    //             "channel_id": "-",
                    //             "self_mute": false,
                    //             "self_deaf": false
                    //         }
                    //     }
                    // "#;
                    
                    // discord_obj.send_gateway_raw(msg).await.unwrap();
                } else {
                    
                    let command: Option<(String, Vec<String>)> = if content.starts_with("!") && !content.starts_with("!tag ") && !content.starts_with("!t ") {
                        let (command, args) = match command_parsing::parse_command(&msg.content) {
                            Ok(res) => res,
                            Err(err) => {
                                // dbg!(&err);
                                let err_text = format!("{}", err);
                                let err_text = err_text.replace("`", "\u{200D}`");
                                let err_text = format!("```{}```", err_text);
                                
                                let send_msg: discord_lib::send_message::NewMessage = err_text.into();
                                
                                let to = msg.channel_id.clone();
                                let fut = send_handle.send(to, &send_msg);
                                if let Err(err) = fut.await {
                                    dbg!("Failed to send message: {:?}", err);
                                    // continue 'msg_recv
                                    // break
                                }
                                
                                // reply!(format!("```{}```", err_text));
                                // continue
                                // panic!();
                                // return
                                continue
                            }
                        };
                        
                        let mut command = command[1..].to_string();
                        command.make_ascii_lowercase();
                        // let command: &str = &command;
                        
                        // let command = command_aliases.get(command).unwrap_or(&command);
                        if let Some(new_command) = command_aliases.get(command.as_str()) {
                            command = new_command.to_string()
                        }
                        
                        let args: Vec<String> = args.iter().map(|x| x.to_string()).collect();
                        
                        let command = command.to_string();
                        Some((command, args))
                    } else {
                        None
                    };
                    
                    let handlers = handlers.clone();
                    let pool = pool.clone();
                    let send_handle = send_handle.clone();
                    
                    tokio::task::spawn(async move {
                        let msg = msg;
                        
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
                        
                        let handler_list: Vec<_> = handlers.values().collect();
                        
                        for handler in handler_list {
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
                                            // continue 'msg_recv
                                            // break
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
                                
                                use handlers::Command;
                                let command = Command {
                                    args: &args,
                                    message: &msg,
                                    pool: &pool,
                                    send_handle: &send_handle,
                                    state: &wrapper.state,
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
                                            // continue 'msg_recv
                                            // break
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
                                
                                
                                let command = Command {
                                    args: &PLACEHOLDER_ARGS,
                                    message: &msg,
                                    pool: &pool,
                                    send_handle: &send_handle,
                                    state: &wrapper.state,
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
                        }
                    });
                }
            }
            GM::Event(E::MessageReactionAdd(reaction)) => {
                if let Some(ref self_id) = self_id {
                    if &reaction.user_id == self_id {
                        continue;
                    }
                }
                
                // eprintln!("reaction {:#?}", reaction);
                let handlers = handlers.clone();
                
                tokio::task::spawn(async move {
                    let handler_list: Vec<_> = handlers.values().collect();
                    
                    for handler in handler_list.iter() {
                        match AssertUnwindSafe(handler.handle_reaction_add_simple(&reaction)).catch_unwind().await {
                            Ok(()) => {}
                            Err(err) => {
                                dbg!(handler, err);
                            }
                        }
                    }
                });
            }
            GM::Event(E::MessageReactionRemove(reaction)) => {
                if let Some(ref self_id) = self_id {
                    if &reaction.user_id == self_id {
                        continue;
                    }
                }
                
                // eprintln!("reaction remove {:?}", reaction);
                let handlers = handlers.clone();
                
                tokio::task::spawn(async move {
                    let handler_list: Vec<_> = handlers.values().collect();
                    
                    for handler in handler_list.iter() {
                        match AssertUnwindSafe(handler.handle_reaction_remove_simple(&reaction)).catch_unwind().await {
                            Ok(()) => {}
                            Err(err) => {
                                dbg!(handler, err);
                            }
                        }
                    }
                });
            }
            
            GM::Hello(_) => {}
            GM::Event(E::Ready(_)) => {
                eprintln!("{:>10}: ready", name);
            }
            GM::Event(E::PresenceUpdate(_)) => {}
            GM::Event(E::VoiceStateUpdate(_)) => {}
            GM::Event(E::Unknown(event_type, event_data)) if event_type == "MESSAGE_UPDATE".to_string() => {
                let pool = pool.clone();
                // let handlers = handlers.clone();
                
                // dbg!(&event_data);
                tokio::task::spawn(async move {
                    let res = AssertUnwindSafe((|| async {
                        let event_data = event_data.as_object().unwrap();
                        
                        let content = event_data.get("content")
                            .and_then(|v| v.as_str());
                        let content = match content {
                            Some(x) => x,
                            None => return,
                        };
                        // if !(content.starts_with("!tag ") || content.starts_with("`")) {
                        //     return
                        // }
                        let tag_name = handlers::tagging::parse_tag_message(content);
                        let tag_name = match tag_name {
                            Some(x) => x,
                            None => return,
                        };
                        
                        let message_id: u64 = event_data.get("id").unwrap().as_str().unwrap().parse().unwrap();
                        
                        let res = sqlx::query(r#"
                            UPDATE tags.tags
                                SET "name" = $2
                                WHERE message_id = $1
                        "#)
                            .bind(to_i(message_id))
                            .bind(tag_name)
                            .execute(&pool).await;
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
            GM::Event(E::Unknown(event_type, event_data)) if event_type == "MESSAGE_DELETE".to_string() => {
                let pool = pool.clone();
                
                tokio::task::spawn(async move {
                    let res = AssertUnwindSafe((|| async {
                        let event_data = event_data.as_object().unwrap();
                        
                        let message_id: u64 = event_data.get("id").unwrap().as_str().unwrap().parse().unwrap();
                        
                        let res = sqlx::query(r#"
                            DELETE FROM tags.tags
                                WHERE message_id = $1
                        "#)
                            .bind(to_i(message_id))
                            .execute(&pool).await;
                        if let Err(err) = res {
                            eprintln!("edit tag {:?}", err);
                        }
                    })()).catch_unwind().await;
                    if let Err(err) = res {
                        dbg!(err);
                    }
                });
            }
            GM::Event(E::Unknown(event_type, _)) if event_type == "MESSAGE_REACTION_REMOVE_ALL".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "MESSAGE_DELETE_BULK".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "GUILD_MEMBER_ADD".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "GUILD_MEMBER_REMOVE".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "GUILD_MEMBER_UPDATE".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "GUILD_BAN_ADD".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "GUILD_EMOJIS_UPDATE".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "GUILD_ROLE_CREATE".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "GUILD_ROLE_UPDATE".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "GUILD_ROLE_DELETE".to_string() => {}
            GM::Event(E::Unknown(event_type, event_data)) if event_type == "GUILD_CREATE".to_string() => {
                let event_data = event_data.as_object().unwrap();
                // owner_id
                // id
                let guild_id: u64 = event_data.get("id").unwrap().as_str().unwrap().parse().unwrap();
                let owner_id: u64 = event_data.get("owner_id").unwrap().as_str().unwrap().parse().unwrap();
                
                let pool = pool.clone();
                tokio::task::spawn(async move {
                    let res = sqlx::query(r#"
                        INSERT INTO config.server_admins (server, "group", readable) VALUES (
                            $1,
                            $2,
                            NULL
                        )
                        ON CONFLICT DO NOTHING;
                    "#)
                        .bind(to_i(guild_id))
                        .bind(to_i(owner_id))
                        .execute(&pool).await;
                    
                    if let Err(e) = res {
                        eprintln!("owner setup {:?}", e);
                    }
                });
            }
            GM::Event(E::Unknown(event_type, _event_data)) if event_type == "GUILD_DELETE".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "GUILD_UPDATE".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "GUILD_BAN_REMOVE".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "TYPING_START".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "CHANNEL_PINS_UPDATE".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "CHANNEL_UPDATE".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "CHANNEL_CREATE".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "CHANNEL_DELETE".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "INVITE_CREATE".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "INVITE_DELETE".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "GUILD_JOIN_REQUEST_DELETE".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "INTEGRATION_CREATE".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "WEBHOOKS_UPDATE".to_string() => {}
            GM::Event(E::Unknown(event_type, _)) if event_type == "RESUMED".to_string() => {
                eprintln!("{:>10}: resumed", name);
            }
            GM::Raw(_msg) => {
                // eprintln!("\n\n?RAW {:?}", _msg);
            }
            _other => {
                // eprintln!("\n\n?OTHER {:?}", _other);
            }
        }
    }
}

use sqlx::migrate::Migrator;
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

async fn migrate() -> Result<(), sqlx::Error> {
    use sqlx::migrate::MigrateError;
    
    let pool = get_pool().await.unwrap();
    
    let res = MIGRATOR
        .run(&pool)
        .await;
    
    if let Err(MigrateError::VersionMismatch(_)) = res {
        dbg!(&res);
        return Ok(())
    }
    
    res?;
    
    Ok(())
}

async fn mark_migrated(version: i64) -> Result<(), sqlx::Error> {
    
    let migration = match MIGRATOR.iter().find(|x| x.version == version) {
        Some(m) => m,
        None => {
            eprintln!("no migration matching version {}", version);
            return Ok(())
        }
    };
    
    let pool = get_pool().await.unwrap();
    
    let mut transaction = pool.begin().await?;
    
    sqlx::query(r#"
            INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
            VALUES ($1, $2, true, $3, 0)
            ON CONFLICT ("version")
                DO UPDATE SET
                    success = true,
                    checksum = $3
        "#)
        .bind(version)
        .bind(&*migration.description)
        .bind(&*migration.checksum)
        .execute(&mut *transaction).await?;
    
    transaction.commit().await?;
    
    Ok(())
}

fn to_i(x: u64) -> i64 {
    i64::from_be_bytes(x.to_be_bytes())
}

fn from_i(x: i64) -> u64 {
    u64::from_be_bytes(x.to_be_bytes())
}

async fn manage_admin(op: &str, user: u64) -> Result<(), sqlx::Error> {
    let pool = get_pool().await.unwrap();
    
    match op {
        "add" => {
            sqlx::query(r#"
                INSERT INTO config.admins ("group", readable) VALUES (
                    $1,
                    NULL
                )
                ON CONFLICT DO NOTHING;
            "#)
                .bind(to_i(user))
                .execute(&pool).await?;
        }
        "rem" => {
            sqlx::query(r#"
                DELETE FROM config.admins
                    WHERE "group" = $1
            "#)
                .bind(to_i(user))
                .execute(&pool).await?;
        }
        "clear" => {
            sqlx::query(r#"
                DELETE FROM config.admins
            "#)
                .bind(to_i(user))
                .execute(&pool).await?;
        }
        _ => panic!("unknown op")
    }
    
    Ok(())
}

fn main() {
    let mut args = std::env::args().skip(1);
    match args.next().as_ref().map(|x| x.as_str()) {
        Some("init") => {
            // let _ = DB.try_load_all();
            // DB.save_all().unwrap();
            return
        }
        Some("parse") => {
            let cmd = args.next().unwrap();
            
            match command_parsing::parse_command(&cmd) {
                Ok((command, args)) => {
                    dbg!(&command, &args);
                }
                Err(err) => {
                    println!("{}", err);
                }
            }
            
            return
        }
        Some("migrate") => {
            // use discord_lib::tokio::runtime::Runtime;
            let rt = Runtime::new().unwrap();
            rt.block_on(migrate()).unwrap();
            // migrate();
            return
        }
        Some("mark_migrated") => {
            let rt = Runtime::new().unwrap();
            let version: i64 = args.next().unwrap().parse().unwrap();
            rt.block_on(mark_migrated(version)).unwrap();
            return
        }
        Some("admin") => {
            let op = args.next().unwrap();
            let user: u64 = args.next().unwrap().parse().unwrap();
            
            // use discord_lib::tokio::runtime::Runtime;
            let rt = Runtime::new().unwrap();
            rt.block_on(manage_admin(&op, user)).unwrap();
            return
        }
        Some(_) => {
            println!("invalid arg");
            return
        }
        _ => {}
    }
    
    let rt = Runtime::new().unwrap();
    rt.block_on(discord_stuff());
}
