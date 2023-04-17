
use discord_lib::discord;
// use rustbreak::Database;
use rustbreak::FileDatabase;
use rustbreak::deser::Ron;
use rustbreak::error::RustbreakError;
use std::collections::{ HashMap, HashSet, BTreeMap, BTreeSet };
// use std::time::SystemTime;
use serde::{Serialize, Deserialize};
use chrono::{ DateTime, FixedOffset };
use discord_lib::tokio::time::delay_for;
use discord_lib::tokio;
use discord_lib::futures::FutureExt as _;
use std::panic::AssertUnwindSafe;
// use discord_lib::send_message::NewMessage;
use discord_lib::tokio::runtime::Runtime;

mod auto_stream_live;
mod auto_chooks;
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
mod members;
mod util;
mod server_settings;
mod auto_live;

use discord_lib::futures::lock::Mutex;
use std::sync::Arc;
use discord_lib::discord::Snowflake;
use discord_lib::Channel;
use discord_lib::SendHandle;

use sqlx::PgPool;

type DateTimeF = DateTime<FixedOffset>;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Tag {
    name: String,
    time: DateTimeF,
    #[serde(default)]
    user: u64,
    #[serde(default)]
    message_id: u64,
    #[serde(default)]
    votes: u64,
    #[serde(default)]
    deleted: bool,
    #[serde(default)]
    adjustments: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Offset {
    position: i64,
    offset: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Stream {
    tags: Vec<Tag>,
    offsets: Vec<Offset>,
    start_time: DateTimeF,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct State {
    streams: HashMap<String, Stream>,
    current_stream: HashMap<u64, String>,
    subscriptions: HashMap<u64, Vec<String>>,
    // session_id: Option<String>,
    // seq: Option<u64>,
    // self_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigState {
    admin_perm: HashMap<u64, BTreeSet<u64>>,
}

impl ConfigState {
    fn new() -> Self {
        ConfigState {
            admin_perm: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionState {
    session_id: Option<String>,
    seq: Option<u64>,
    self_id: Option<u64>,
    #[serde(default)]
    last_member_sync: Option<DateTimeF>,
    #[serde(default)]
    active: HashSet<String>,
    #[serde(default)]
    chooks_active: HashSet<String>,
    #[serde(default)]
    guilds: HashSet<u64>,
}

impl SessionState {
    fn new() -> Self {
        SessionState {
            session_id: None,
            seq: None,
            self_id: None,
            last_member_sync: None,
            active: HashSet::new(),
            chooks_active: HashSet::new(),
            guilds: HashSet::new(),
        }
    }
}

type DatabaseInner = FileDatabase<State, Ron>;
type ConfigStateDb = FileDatabase<ConfigState, Ron>;
pub(crate) type SessionStateDb = FileDatabase<SessionState, Ron>;

struct Database {
    inner: DatabaseInner,
    config_state: ConfigStateDb,
    session_state: SessionStateDb,
}

impl Database {
    // async fn async_save_data(&self) -> Result<(), RustbreakError> {
    //     self.inner.save()
    //     // discord_lib::tokio::task::spawn_blocking(|| {
    //     //     // self.inner.save()
    //     //     self.inner.save()
    //     // }).await.expect("save tokio spawn_blocking failed")
    // }
    
    // async fn async_save_config(&self) -> Result<(), RustbreakError> {
    //     self.config_state.save()
    //     // discord_lib::tokio::task::spawn_blocking(|| {
    //     //     // self.inner.save()
    //     //     self.config_state.save()
    //     // }).await.expect("save tokio spawn_blocking failed")
    // }
    
    // async fn async_save_session(&self) -> Result<(), RustbreakError> {
    //     self.session_state.save()
    //     // discord_lib::tokio::task::spawn_blocking(|| {
    //     //     // self.inner.save()
    //     //     self.session_state.save()
    //     // }).await.expect("save tokio spawn_blocking failed")
    // }
    
    // fn save_all(&self) -> Result<(), RustbreakError> {
    //     self.inner.save()?;
    //     self.config_state.save()?;
    //     self.session_state.save()?;
        
    //     Ok(())
    // }
    
    fn load_all(&self) -> Result<(), RustbreakError> {
        self.config_state.load()?;
        self.inner.load()?;
        self.session_state.load()?;
        
        Ok(())
    }
    
    // fn try_load_all(&self) -> Result<(), RustbreakError> {
    //     let _ = self.config_state.load();
    //     let _ = self.inner.load();
    //     let _ = self.session_state.save();
        
    //     Ok(())
    // }
}

impl std::ops::Deref for Database {
    type Target = DatabaseInner;
    
    fn deref(&self) -> &DatabaseInner {
        &self.inner
    }
}

fn get_db() -> Database {
    let new_data = State {
        streams: HashMap::new(),
        current_stream: HashMap::new(),
        subscriptions: HashMap::new(),
        // session_id: None,
        // seq: None,
        // self_id: None,
    };
    // new_data.streams.insert("".into(), Stream {
    //     tags: Vec::new(),
    //     start_time: SystemTime::now(),
    // });
    
    // Database::open("./database.ron").unwrap()
    let db = FileDatabase::from_path("./kq_db.ron", new_data).unwrap();
    let config_db = FileDatabase::from_path("./kq_config.ron", ConfigState::new()).unwrap();
    let session_db = FileDatabase::from_path("./kq_session.ron", SessionState::new()).unwrap();
    // match db.load() {
    //     Ok(()) => (),
    //     Err(err) => {
    //         println!("failed to load db {:?}", err);
    //         panic!();
    //     }
    // }
    Database {
        inner: db,
        config_state: config_db,
        session_state: session_db,
    }
}

// lazy_static::lazy_static! {
//     static ref DB: Database = {
//         get_db()
//     };
// }

use discord_lib::discord::Message;
fn get_mention_ids(msg: &Message) -> Result<Vec<u64>, &str> {
    let ids = if msg.mentions.is_empty() && msg.mention_roles.is_empty() {
        let new_id: &str = msg.content.splitn(3, " ").skip(2).next()
            .ok_or("Incorrect parameters")?;
        
        let new_id: u64 = match new_id.parse() {
            Ok(id) => id,
            Err(_) => {
                return Err("Invalid id format");
                // reply!("Invalid id format");
                
                // continue
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
    Sqlx(sqlx::Error),
}

async fn get_pool() -> Result<PgPool, GetPoolError> {
    let pg_url = match std::env::var("pg_url") {
        Ok(token) => token,
        Err(_e) => {
            // eprintln!("Failed to get env var pg_url: {:?}", e);
            return Err(GetPoolError::MissingEnv);
        }
    };
    
    use sqlx::postgres::PgPoolOptions;
    
    let pool: PgPool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&pg_url).await.map_err(|e| GetPoolError::Sqlx(e))?;
    
    Ok(pool)
}

#[derive(Debug)]
pub struct DiscordState {
    // list of guilds the bot is in
    servers: BTreeSet<Snowflake>,
    channel_cache: BTreeMap<u64, Channel>,
    send_handle: Option<SendHandle>,
}

impl DiscordState {
    pub fn new_session(&mut self) {
        self.servers.clear();
    }
    
    pub async fn get_channel(&mut self, channel_id: Snowflake) -> Result<Channel, anyhow::Error> {
        let channel_id = channel_id.0;
        
        let get_res = {
            // let mut state = d_state.lock().await;
            let channel_cache = &mut self.channel_cache;
            channel_cache.get(&channel_id).map(|x| x.clone())
        };
        
        let send_handle = self.send_handle.as_ref()
            .ok_or_else(|| anyhow::anyhow!("DState send handle is None"))?;
        
        let channel = match get_res {
            Some(channel) => channel,
            None => {
                let channel_sf = Snowflake(channel_id);
                let channel = match send_handle.get_channel(channel_sf).await {
                        // .map_err(|e| {
                        //     println!("channel failed {}", channel_id);
                        //     e
                        // })
                        // .some_error("get channel") {
                    Ok(channel) => channel,
                    Err(err) => {
                        println!("channel failed {} {:?}", channel_id, err);
                        // continue
                        return Err(anyhow::anyhow!("channel failed {} {:?}", channel_id, err))
                    }
                };
                // println!("{:#?}", channel);
                {
                    // let mut state = d_state.lock().await;
                    let channel_cache = &mut self.channel_cache;
                    let channel = channel_cache.entry(channel_id).or_insert(channel);
                    channel.clone()
                }
            }
        };
        
        Ok(channel)
    }
}

pub type DState = Arc<Mutex<DiscordState>>;

// const BASE_URL: &'static str = "https://discordapp.com/api/v6";
const BASE_URL: &'static str = "https://discord.com/api/v9";

async fn discord_stuff() {
    
    let d_state = DiscordState {
        servers: BTreeSet::new(),
        channel_cache: BTreeMap::new(),
        send_handle: None,
    };
    let d_state = Arc::new(Mutex::new(d_state));
    
    let mut self_id: Option<discord::Snowflake> = None;
    let mut last_seq: Option<u64> = None;
    // let mut reconnecting = false;
    
    let token = match std::env::var("discord_auth") {
        Ok(token) => token,
        Err(e) => {
            eprintln!("Failed to get env var discard_auth: {:?}", e);
            return;
        }
    };
    // load to throw error early if environment variable isn't set
    let _ = &*i_love_youtube::YT_API_KEY;
    let holodex_api_key = std::env::var("holodex_api_key").expect("environment variable holodex_api_key not set");
    
    // let pg_url = match std::env::var("pg_url") {
    //     Ok(token) => token,
    //     Err(e) => {
    //         eprintln!("Failed to get env var pg_url: {:?}", e);
    //         return;
    //     }
    // };
    
    // use sqlx::PgPool;
    // use sqlx::postgres::PgPoolOptions;
    
    // let pool: PgPool = PgPoolOptions::new()
    //     .max_connections(5)
    //     .connect(&pg_url).await.unwrap();
    
    let pool = match get_pool().await {
        Ok(pool) => pool,
        Err(GetPoolError::MissingEnv) => {
            eprintln!("Failed to get env var pg_url");
            return;
        }
        Err(e) => panic!("{:?}", e),
    };
    
    let db = get_db();
    let _ = db.session_state.load();
    let session_state = Arc::new(db.session_state);
    
    let mut discord_obj = {
        let mut data = session_state.borrow_data_mut().unwrap();
        match (&data.session_id, &data.seq) {
            (Some(ref session_id), Some(seq)) => {
                self_id = data.self_id.map(|x| discord::Snowflake(x));
                last_seq = data.seq;
                
                discord_lib::Discord::reconnect_with(BASE_URL.into(), token.clone(), session_id.clone(), *seq).await.unwrap()
            }
            _ => {
                data.session_id = None;
                data.seq = None;
                discord_lib::Discord::connect(BASE_URL.into(), token.clone()).await.unwrap()
            }
        }
    };
    
    let send_handle = discord_obj.get_send_handle();
    {
        let mut d_state = d_state.lock().await;
        d_state.send_handle = Some(send_handle.clone());
    }
    let task_send_handle = send_handle.clone();
    let task_pool = pool.clone();
    let task_session_state = Arc::clone(&session_state);
    let task_d_state = d_state.clone();
    // let task = auto_stream_live::auto_live_task(send_handle);
    // let _task_handle = discord_lib::tokio::task::spawn(task);
    let _task_handle = discord_lib::tokio::task::spawn(async move {
        let session_state = task_session_state;
        // let mut active: HashSet<String> = HashSet::new();
        let mut active = {
            let data = session_state.borrow_data().unwrap();
            
            data.active.clone()
            // data.active = active.clone();
        };
        
        loop {
            let task = auto_live::holodex::auto_live_task(&task_send_handle, &mut active, &task_pool, task_d_state.clone(), &holodex_api_key);
            // let task = auto_stream_live::auto_live_task(&task_send_handle, &mut active, &task_pool, task_d_state.clone());
            
            match task.await {
                Ok(()) => {
                    // println!("active save start");
                    // delay_for(::std::time::Duration::new(30, 0)).await;
                    {
                        let mut data = session_state.borrow_data_mut().unwrap();
                        
                        data.active = active.clone();
                    }
                    // db.async_save_session().await.unwrap();
                    session_state.save().unwrap();
                    // println!("active save end");
                },
                Err(err) => {
                    eprintln!("auto live task failed {:?}", err);
                }
            }
            
            delay_for(::std::time::Duration::new(30, 0)).await;
        }
    });
    
    let task_send_handle = send_handle.clone();
    let task_pool = pool.clone();
    let task_session_state = Arc::clone(&session_state);
    let task_d_state = d_state.clone();
    // let task = auto_stream_live::auto_live_task(send_handle);
    // let _task_handle = discord_lib::tokio::task::spawn(task);
    if false {
        let _task_handle = discord_lib::tokio::task::spawn(async move {
            let session_state = task_session_state;
            // let mut active: HashSet<String> = HashSet::new();
            let mut active = {
                let data = session_state.borrow_data().unwrap();
                
                data.chooks_active.clone()
                // data.active = active.clone();
            };
            
            loop {
                use auto_chooks::run_chooks;
                let task = run_chooks(&mut active, &task_send_handle, &task_pool, task_d_state.clone());
                match task.await {
                    Ok(()) => {
                        // println!("active save start");
                        // delay_for(::std::time::Duration::new(30, 0)).await;
                        {
                            let mut data = session_state.borrow_data_mut().unwrap();
                            
                            data.chooks_active = active.clone();
                        }
                        // db.async_save_session().await.unwrap();
                        session_state.save().unwrap();
                        // println!("active save end");
                    },
                    Err(err) => {
                        eprintln!("auto chooks task failed {:?}", err);
                    }
                }
                
                delay_for(::std::time::Duration::new(30, 0)).await;
            }
        });
    }
    
    if false { // disable membership job
        let task_send_handle = send_handle.clone();
        let task_pool = pool.clone();
        let task_session_db = session_state.clone();
        let _task_handle = discord_lib::tokio::task::spawn(async move {
            // let mut active: HashSet<String> = HashSet::new();
            
            loop {
                let task = crate::members::bg_task::
                    member_task(
                        task_session_db.clone(),
                        task_pool.clone(),
                        task_send_handle.clone(),
                    );
                match task.await {
                    Ok(()) => (),
                    Err(err) => {
                        eprintln!("member update task failed {:?}", err);
                    }
                }
                delay_for(::std::time::Duration::new(240, 0)).await;
            }
        });
    }
    
    // let _task_handle = discord_lib::tokio::task::spawn(async move {
    
    // let b = discord_lib::tokio::task::spawn(async move {
    //     eprintln!("task handle.await {:?}", task_handle.await);
    //     // if let Err(err) = task_handle.await {
    //     //     eprintln!("err in checker task {:?}", err);
    //     // }
    // });
    // b.await.unwrap();
    // return;
    
    // let mut self_id: Option<discord::Snowflake> = None;
    
    
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
    use handlers::streams::{ OffsetHandler, YtStartHandler, TwitchStartHandler, TwitterSpaceStartHandler, SetStreamHandler };
    use handlers::config::{ SubscribeHandler, ManageAdminHandler };
    use handlers::jank::{ CopyTagsHandler };
    use members::handlers::{ MembersHandler, MembersAdminHandler, VerifyHandler };
    use auto_live::channel_watch::ChannelWatchHandler;
    // use help_text::HelpHandler;
    
    handlers.insert("xpotato", Box::new(XPotatoHandler {}));
    handlers.insert("tags", Box::new(TagsHandler { pool: pool.clone() }));
    handlers.insert("tag", Box::new(TagHandler { vote_emoji: "⭐".into(), delete_emoji: "❌".into(), pool: pool.clone() }));
    handlers.insert("test", Box::new(TestHandler {}));
    handlers.insert("offset", Box::new(OffsetHandler { pool: pool.clone() }));
    handlers.insert("adjust", Box::new(AdjustHandler { pool: pool.clone() }));
    handlers.insert("copy_tags", Box::new(CopyTagsHandler { pool: pool.clone() }));
    // handlers.insert("recreate", Box::new(RecreateHandler {}));
    handlers.insert("yt_start", Box::new(YtStartHandler { pool: pool.clone() }));
    handlers.insert("twitch_start", Box::new(TwitchStartHandler { pool: pool.clone() }));
    handlers.insert("spaces_start", Box::new(TwitterSpaceStartHandler { pool: pool.clone() }));
    // handlers.insert("streams", Box::new(ListStreamsHandler { pool: pool.clone() }));
    handlers.insert("stream", Box::new(SetStreamHandler { pool: pool.clone() }));
    handlers.insert("sub", Box::new(SubscribeHandler { pool: pool.clone() }));
    handlers.insert("admin", Box::new(ManageAdminHandler { pool: pool.clone() }));
    handlers.insert("members", Box::new(MembersHandler { pool: pool.clone() }));
    handlers.insert("membersa", Box::new(MembersAdminHandler { pool: pool.clone() }));
    handlers.insert("verify", Box::new(VerifyHandler { pool: pool.clone() }));
    
    handlers.insert("watch_channel", Box::new(ChannelWatchHandler::new(pool.clone(), d_state.clone())));
    
    // handlers.insert("help", Box::new(HelpHandler {
    //     handlers: Arc::new(handlers.clone()),
    //     aliases: BTreeMap::new(),
    //     prefix: "!",
    // }));
    
    // let histogram = HistogramHandler::new();
    // handlers.insert("histo_rebuild", Box::new(histogram.rebuild_handler(format!("Bot {}", token))));
    // handlers.insert("histo", Box::new(histogram));
    
    let mut command_aliases = BTreeMap::new();
    command_aliases.insert("t", "tag");
    
    let handlers = Arc::new(handlers);
    
    
    
    // let send_handle = discord_obj.get_send_handle();
    
    // 'msg_recv: loop {
    loop {
        // dbg!("loop start");
        use discord_lib::gateway::GatewayError;
        let msg = match discord_obj.recv().await {
            Ok(msg) => msg,
            Err(err) => {
                eprintln!("Err recieving message: {:?}", err);
                
                if let GatewayError::InvalidSession = err {
                    eprintln!("InvalidSession: ");
                    {
                        let d_state = &mut *d_state.lock().await;
                        d_state.new_session();
                    }
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
                // discord_obj = loop {
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
                            delay_for(::std::time::Duration::new(wait_secs, 0)).await;
                            if wait_secs < 20 {
                                wait_secs *= 2;
                            }
                        }
                    }
                };
                continue
            },
        };
        // dbg!(&msg);
        
        let new_seq = discord_obj.seq();
        if new_seq != last_seq {
            last_seq = new_seq;
            {
                let mut data = session_state.borrow_data_mut().unwrap();
                data.seq = new_seq;
            }
            // db.async_save_session().await.unwrap();
            session_state.save().unwrap();
        }
        
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
                    let fut = discord_obj.send(to, &send_msg);
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
                    
                    
                    
                    
                    
                    // let content = content.clone();
                    let send_handle = discord_obj.get_send_handle();
                    
                    // let handlers = handlers.clone();
                    
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
                    
                    // let command = command.to_string();
                    // let command = "".to_string();
                    // let args: Vec<_> = args.iter().cloned().collect();
                    // let args: Vec<String> = Vec::new();
                    // let content: String = content.clone();
                    // let msg = msg.clone();
                    
                    // let handler = handlers.get(command).unwrap();
                    // let handler = handlers.get(command).unwrap();
                    
                    let handlers = handlers.clone();
                    let pool = pool.clone();
                    let send_handle = send_handle.clone();
                    
                    tokio::task::spawn(async move {
                        let msg = msg;
                        // let content = content;
                        // let ref content = msg.content;
                        
                        
                        
                        // let (command, args) = match command_parsing::parse_command(content) {
                        //     Ok(res) => res,
                        //     Err(err) => {
                        //         // dbg!(&err);
                        //         let err_text = format!("{}", err);
                        //         let err_text = err_text.replace("`", "\u{200D}`");
                        //         let err_text = format!("```{}```", err_text);
                                
                        //         let send_msg: discord_lib::send_message::NewMessage = err_text.into();
                                
                        //         let to = msg.channel_id.clone();
                        //         let fut = send_handle.send(to, &send_msg);
                        //         if let Err(err) = fut.await {
                        //             dbg!("Failed to send message: {:?}", err);
                        //             // continue 'msg_recv
                        //             // break
                        //         }
                                
                        //         // reply!(format!("```{}```", err_text));
                        //         // continue
                        //         // panic!();
                        //         return
                        //     }
                        // };
                        // dbg!(&command, &args);
                        
                        
                        
                        // let mut handlers:
                        //     BTreeMap<&'static str, Box<dyn Handler>>
                        //     = BTreeMap::new();
                        
                        // use handlers::{
                        //     Handler, HandlerResult,
                        // };
                        
                        
                        // handler.handle_command(&args, &msg);
                        
                        
                        // let command = &command[1..];
                        
                        
                        
                        use crate::handlers::HandlerResponse;
                        // use discord_lib::SendHandle;
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
                                            // continue 'msg_recv
                                            // break
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
            GM::Event(E::Ready(ready)) => {
                {
                    let mut data = session_state.borrow_data_mut().unwrap();
                    data.session_id = Some(ready.session_id.clone());
                    data.self_id = Some(ready.user.id.0);
                }
                // db.async_save_session().await.unwrap();
                session_state.save().unwrap();
                self_id = Some(ready.user.id);
                let current_guilds = {
                    let d_state = &mut *d_state.lock().await;
                    for guild in ready.guilds.iter() {
                        d_state.servers.insert(guild.id.clone());
                    }
                    d_state.servers.iter().map(|x| x.0).collect()
                };
                // dbg!(&current_guilds);
                {
                    let mut data = session_state.borrow_data_mut().unwrap();
                    data.guilds = current_guilds;
                }
                session_state.save().unwrap();
            }
            GM::Hello(_) => {}
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
                        
                        // pool;
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
                // eprintln!("{:#?}", event_data);
                let event_data = event_data.as_object().unwrap();
                // owner_id
                // id
                let guild_id: u64 = event_data.get("id").unwrap().as_str().unwrap().parse().unwrap();
                let owner_id: u64 = event_data.get("owner_id").unwrap().as_str().unwrap().parse().unwrap();
                
                let current_guilds = {
                    let d_state = &mut *d_state.lock().await;
                    d_state.servers.insert(Snowflake(guild_id));
                    
                    d_state.servers.iter().map(|x| x.0).collect()
                };
                // dbg!(&current_guilds);
                {
                    let mut data = session_state.borrow_data_mut().unwrap();
                    data.guilds = current_guilds;
                }
                session_state.save().unwrap();
                
                // {
                //     let mut data = DB.config_state.borrow_data_mut().unwrap();
                    
                //     let admins = data.admin_perm
                //         .entry(guild_id)
                //         .or_insert_with(|| BTreeSet::new());
                    
                //     admins.insert(owner_id);
                //     admins.insert(-);
                // }
                // DB.async_save_config().await.unwrap();
                
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
            GM::Event(E::Unknown(event_type, event_data)) if event_type == "GUILD_DELETE".to_string() => {
                let event_data = event_data.as_object().unwrap();
                
                let guild_id: u64 = event_data.get("id").unwrap().as_str().unwrap().parse().unwrap();
                
                let current_guilds = {
                    let d_state = &mut *d_state.lock().await;
                    d_state.servers.remove(&Snowflake(guild_id));
                    // update_db_from_state(&session_state, d_state).unwrap();
                    d_state.servers.iter().map(|x| x.0).collect()
                };
                dbg!(&current_guilds);
                {
                    let mut data = session_state.borrow_data_mut().unwrap();
                    data.guilds = current_guilds;
                }
                session_state.save().unwrap();
            }
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
                println!("resumed");
                let guilds = {
                    let data = session_state.borrow_data().unwrap();
                    data.guilds.iter().map(|x| Snowflake(*x)).collect()
                };
                let d_state = &mut *d_state.lock().await;
                // dbg!(&d_state.servers);
                // assert!(d_state.servers.is_empty());
                // dbg!(&guilds);
                // if d_state.servers.is_empty() { // if servers isn't empty the program wasn't restarted
                d_state.servers = guilds;
                // }
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

// fn test_glue() {
//     use gluesql::{parse, Glue, SledStorage};
//     use sqlparser::ast::{Statement, Expr, SetExpr, Values, Value};
    
//     let storage = SledStorage::new("data.db").unwrap();
//     let mut glue = Glue::new(storage);
    
//     let sqls = r#"
//         -- CREATE TABLE Glue (id INTEGER, "name" text,);
//         -- CREATE TABLE Glue (id INTEGER DEFAULT nextval(''::regclass), "name" text,);
//         CREATE TABLE Glue (id INTEGER DEFAULT nextval(''::regclass), UNIQUE ("id"));
//         INSERT INTO Glue VALUES (100);
//         INSERT INTO Glue VALUES (100);
//         INSERT INTO Glue VALUES (200);
//         -- INSERT INTO Glue VALUES (max(Glue.id));
//         SELECT * FROM Glue WHERE id > 100;
//         -- D ROP TABLE Glue;
//     "#;
    
//     for query in parse(sqls).unwrap() {
//         let res = glue.execute(&query).unwrap();
//         println!("{:#?} {:?}\n", query.0, res);
//     }
    
//     let sqls = "
//         INSERT INTO Glue VALUES (NULL);
//     ";
    
//     let mut query = &mut parse(sqls).unwrap()[0];
//     // let x = query.0.source.body.0;
//     if let Statement::Insert{ ref mut source, .. } = query.0 {
//         if let SetExpr::Values(Values(ref mut values)) = source.body {
//             // let ref mut x = values[0][0];
//             // dbg!(x);
//             *values = vec![
//                 vec![Expr::Value(Value::Number(500.to_string()))],
//                 vec![Expr::Value(Value::Number(600.to_string()))],
//                 // vec![],
//                 // vec![],
//             ];
//         } else {
//             panic!();
//         }
//     } else {
//         panic!();
//     }
//     // dbg!(x);
    
//     let res = glue.execute(&query).unwrap();
//     println!(">>> {:?}", res);
    
//     let sqls = "
//         SELECT * FROM Glue;
//     ";
    
//     for query in parse(sqls).unwrap() {
//         let res = glue.execute(&query).unwrap();
//         println!("{:?}\n", res);
//     }
    
//     let sqls = "
//         DROP TABLE Glue;
//     ";
    
//     for query in parse(sqls).unwrap() {
//         let res = glue.execute(&query).unwrap();
//     }
    
//     return;
// }

// async fn test_pg() -> Result<(), sqlx::Error> {
//     let pool = get_pool().await.unwrap();
    
//     // Make a simple query to return the given parameter
//     let row: (i64,) = sqlx::query_as("SELECT $1")
//         .bind(150_i64)
//         .fetch_one(&pool).await?;
    
//     assert_eq!(row.0, 150);
    
//     let row: (i32, String) = sqlx::query_as(r#"SELECT id, "group"  FROM sensors_1 "#)
//         // .bind(150_i64)
//         .fetch_one(&pool).await?;
    
//     assert_eq!(row.1, "test1");
    
//     Ok(())
// }

use sqlx::migrate::Migrator;
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

async fn migrate() -> Result<(), sqlx::Error> {
    use sqlx::migrate::MigrateError;
    
    // for m in MIGRATOR.iter() {
    //     println!("{:?}", m);
    //     println!("{:x?}", m.checksum);
    // }
    
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
        .execute(&mut transaction).await?;
    
    transaction.commit().await?;
    
    Ok(())
}

// type Query<'q> = sqlx::query::Query<
//     'q, sqlx::Postgres,
//     // <sqlx::Postgres as sqlx::database::HasArguments<'q>>::Arguments
//     sqlx::postgres::PgArguments
// >;

// fn bulk_query<'q, DB, T, F>(data: &'q [T], bind: F) -> sqlx::query::Query<'q, sqlx::Postgres, <sqlx::Postgres as sqlx::database::HasArguments<'q>>::Arguments>
// async fn bulk_query<'q, DB, T, F>(pool: &sqlx::PgPool, data: &'q [T], bind: F) -> Result<(), sqlx::Error>
// async fn bulk_query<'a: 'q, 'q, T: 'a, I: 'q, F>(pool: &sqlx::PgPool, data: &'q mut I, bind: F) -> Result<(), sqlx::Error>
//     where
//         F: Fn(Query, T) -> Query<'a>,
//         I: std::iter::Iterator<Item = T> + std::iter::ExactSizeIterator,
// {
//     let mut query = String::new();
//     query.push_str(r#"INSERT INTO tags.streams (name, start_time) VALUES"#);
    
//     let mut i = 1usize;
//     for _ in 0 .. data.len() {
//         if i != 1 {
//             query.push(',');
//         } else {
//             query.push(' ');
//         }
//         query.push_str(&format!("(${}, ${})", i, i+1));
//         i += 2;
//     }
    
//     // let mut query = sqlx::query(&query);
    
//     // for x in data {
//     //     query = bind(query, x);
//     // }
    
//     // query.execute(pool).await?;
    
//     Ok(())
// }

// trait Insertable {
    
// }

use sqlx::postgres::PgArguments;

struct BulkInsert {
    query_str: String,
    // query: Query<'q>,
    args: PgArguments,
    arg_n: usize,
}

impl BulkInsert {
    fn new(part: &str) -> Self {
        let mut query_str = String::new();
        query_str.push_str(r#"INSERT INTO "#);
        query_str.push_str(part);
        query_str.push_str(r#" VALUES ("#);
        
        
        Self {
            query_str,
            args: PgArguments::default(),
            arg_n: 1,
        }
    }
    
    // fn insert2<'a, T1: 'a, T2: 'a>(&'a mut self, value: T1, value2: T2) 
    //     where
    //         T1: sqlx::prelude::Encode<'a, sqlx::Postgres> + sqlx::prelude::Type<sqlx::Postgres> + Send,
    //         T2: sqlx::prelude::Encode<'a, sqlx::Postgres> + sqlx::prelude::Type<sqlx::Postgres> + Send,
    // {
    //     self.query_str.push_str(&format!("(${}, ${}),", self.arg_n, self.arg_n+1));
    //     use sqlx::Arguments;
    //     self.args.add(value);
    //     self.args.add(value2);
    //     self.arg_n += 2;
    // }
    
    fn add<'a, T: 'a>(&'a mut self, value: T)
        where
            T: sqlx::prelude::Encode<'a, sqlx::Postgres> + sqlx::prelude::Type<sqlx::Postgres> + Send,
    {
        self.query_str.push_str(&format!("${},", self.arg_n));
        use sqlx::Arguments;
        self.args.add(value);
        
        self.arg_n += 1;
    }
    
    fn row(&mut self) {
        if self.query_str.ends_with('(') {
            return
        }
        self.finish_row();
        self.query_str.push('(');
    }
    
    fn finish_row(&mut self) {
        if self.query_str.ends_with(',') {
            self.query_str.pop();
        }
        
        self.query_str.push_str("),");
    }
    
    // async fn run(mut self, pool: &sqlx::PgPool) -> Result<(), sqlx::Error>
    async fn run<'c, E>(mut self, pool: E) -> Result<(), sqlx::Error>
        where
            E: sqlx::prelude::Executor<'c, Database = sqlx::Postgres>
    {
        
        self.finish_row();
        if self.query_str.ends_with(',') {
            self.query_str.pop();
        }
        
        // println!("{}", self.query_str);
        // self.query_str.push_str(r#" RETURNING id"#);
        
        let query = sqlx::query_with(&self.query_str, self.args);
        
        query.execute(pool).await?;
        // use discord_lib::futures::StreamExt;
        // let res: Vec<_> = query.fetch(pool).collect::<Vec<_>>().await;
        // use sqlx::Row;
        // let res: Result<Vec<_>, _> = res.into_iter().collect();
        // let x = res?;
        // // println!("{:?}", x.columns());
        // for x in x.iter() {
        // //     // let x = x?;
        // //     // if let Ok(x) = x {
        //     println!("{:?}", x.columns());
        // //     // }
        // }
        // println!("{:?}", res);
        
        Ok(())
    }
    
    // async fn run_returning<'c, E, T>(mut self, pool: E) -> Result<Vec<T>, sqlx::Error>
    async fn run_returning<'c, E>(mut self, pool: E) -> Result<Vec<sqlx::postgres::PgRow>, sqlx::Error>
        where
            E: sqlx::prelude::Executor<'c, Database = sqlx::Postgres>,
            // T: sqlx::Decode<'_, sqlx::Postgres> + sqlx::Type<sqlx::Postgres> + Clone
    {
        
        self.finish_row();
        if self.query_str.ends_with(',') {
            self.query_str.pop();
        }
        
        self.query_str.push_str(r#" RETURNING id, time"#);
        
        let query = sqlx::query_with(&self.query_str, self.args);
        
        // query.execute(pool).await?;
        use discord_lib::futures::StreamExt;
        let res: Vec<_> = query.fetch(pool).collect::<Vec<_>>().await;
        // use sqlx::Row;
        let res: Result<Vec<_>, _> = res.into_iter().collect();
        let x = res?;
        // println!("{:?}", x.columns());
        // for x in x.iter() {
        // //     // let x = x?;
        // //     // if let Ok(x) = x {
        //     // println!("{:?}", x.columns());
        // //     // }
        // }
        // println!("{:?}", res);
        
        // use std::convert::TryInto;
        let out = x;
        // let mut out: Vec<T> = Vec::new();
        // for y in x {
        //     let z: T = y.try_get(0)?;
        //     // let z = y.into(0);
        //     out.push(z.clone());
        // }
        
        Ok(out)
    }
}

fn to_i(x: u64) -> i64 {
    i64::from_be_bytes(x.to_be_bytes())
}

fn from_i(x: i64) -> u64 {
    u64::from_be_bytes(x.to_be_bytes())
}

async fn migrate_ron() -> Result<(), sqlx::Error> {
    
    let db = get_db();
    
    let pool = get_pool().await.unwrap();
    
    // Make a simple query to return the given parameter
    // let row: (i64,) = sqlx::query_as("SELECT $1")
    //     .bind(150_i64)
    //     .fetch_one(&pool).await?;
    
    // assert_eq!(row.0, 150);
    
    // let row: (i32, String) = sqlx::query_as(r#"SELECT id, "group"  FROM sensors_1 "#)
    //     // .bind(150_i64)
    //     .fetch_one(&pool).await?;
    
    // assert_eq!(row.1, "test1");
    
    db.load_all().unwrap();
    let data = db.borrow_data().unwrap();
    let config_data = db.config_state.borrow_data().unwrap();
    
    // let insert_stream = sqlx::query!(r#"INSERT INTO tags.streams (name, start_time) VALUES ($1, $2)"#);
    
    // bulk_query(&pool, &mut data.streams.iter(), |query, x| {
    //     query
    //         .bind(x.0)
    //         .bind(x.1.start_time)
    // }).await?;
    
    let mut transaction = pool.begin().await?;
    
    let mut stream_insert = BulkInsert::new("tags.streams (name, has_server, server, start_time)");
    for (stream_name, stream) in data.streams.iter() {
        stream_insert.row();
        stream_insert.add(stream_name);
        stream_insert.add(false);
        stream_insert.add(0);
        stream_insert.add(stream.start_time);
    }
    stream_insert.run(&mut transaction).await?;
    
    
    for (stream_name, stream) in data.streams.iter() {//.filter(|(_, s)| !s.tags.is_empty()).take(100) {
        if !stream.tags.is_empty() {
            println!("a1");
            let (stream_id,): (i32,) = sqlx::query_as(r#"SELECT id FROM tags.streams WHERE name = $1"#)
                .bind(stream_name)
                .fetch_one(&mut transaction).await?;
            println!("a2");
            
            for (i, offset) in stream.offsets.iter().enumerate() {
                // dbg!(&offset);
                // println!("a");
                // let x = chrono::Duration::seconds(offset.position);
                // dbg!(&x);
                // println!("b");
                // let y = chrono::Duration::seconds(offset.offset);
                // dbg!(&y);
                // println!("c");
                
                // let x = time::Duration::seconds(offset.position);
                // let y = time::Duration::seconds(offset.offset);
                
                sqlx::query(r#"INSERT INTO tags.stream_offsets ("order", stream, position, "offset") VALUES ($1, $2, $3, $4)"#)
                    .bind(i as i32)
                    .bind(stream_id)
                    .bind(time::Duration::seconds(offset.position))
                    .bind(time::Duration::seconds(offset.offset))
                    // .bind(chrono::Duration::seconds(offset.position))
                    // .bind(chrono::Duration::seconds(offset.offset))
                    .execute(&mut transaction).await?;
                
                println!("d");
            }
            
            let mut tag_insert = BulkInsert::new(r#"tags.tags (stream, "name", "time", "server", "user", message_id, votes, deleted)"#);
            for tag in stream.tags.iter() {
                // println!("{:?}", tag.time.naive_utc());
                
                tag_insert.row();
                tag_insert.add(stream_id);
                tag_insert.add(&tag.name);
                tag_insert.add(&tag.time.naive_utc());
                tag_insert.add(None::<i64>);
                tag_insert.add(to_i(tag.user));
                tag_insert.add(to_i(tag.message_id));
                tag_insert.add(tag.votes as i32);
                tag_insert.add(tag.deleted);
            }
            let res: Vec<_> = tag_insert.run_returning(&mut transaction).await?;
            println!("after run returning");
            use sqlx::Row;
            // let x: i32 = res[0].try_get(0)?;
            
            use chrono::Timelike;
            use chrono::naive::NaiveDateTime;
            
            let res: Result<Vec<(i32, NaiveDateTime)>, sqlx::Error> = res.iter().map(|r| Ok((r.try_get(0)?, r.try_get(1)?))).collect();
            println!("row got");
            let res = res?;
            println!("row good");
            
            assert_eq!(stream.tags.len(), res.len());
            for (tag, (tag_id, tag_time)) in stream.tags.iter().zip(res.iter()) {
                assert_eq!(tag.time.naive_utc().with_nanosecond(0), tag_time.with_nanosecond(0));
                for (i, adj) in tag.adjustments.iter().enumerate() {
                    sqlx::query(r#"INSERT INTO tags.tag_offsets ("order", tag, "offset") VALUES ($1, $2, $3)"#)
                        .bind(i as i32)
                        .bind(tag_id)
                        .bind(time::Duration::seconds(*adj))
                        .execute(&mut transaction).await?;
                }
            }
        }
    }
    
    for (channel_id, stream_name) in data.current_stream.iter() {
        sqlx::query(r#"
            INSERT INTO config.selected_streams (channel, stream) VALUES (
                $1,
                (
                    SELECT "id" FROM tags.streams WHERE name=$2
                )
            )
            ON CONFLICT DO NOTHING;
        "#)
            .bind(to_i(*channel_id))
            .bind(stream_name)
            .execute(&mut transaction).await?;
    }
    
    for (channel_id, yt_channel_ids) in data.subscriptions.iter() {
        for yt_channel_id in yt_channel_ids.iter() {
            sqlx::query(r#"
                INSERT INTO config.subscriptions (channel, sub_id, "type") VALUES (
                    $1,
                    $2,
                    'youtube'
                )
                ON CONFLICT DO NOTHING;
            "#)
                .bind(to_i(*channel_id))
                .bind(yt_channel_id)
                .execute(&mut transaction).await?;
        }
    }
    
    for (server_id, admins) in config_data.admin_perm.iter() {
        for admin in admins {
            sqlx::query(r#"
                INSERT INTO config.server_admins (server, "group", readable) VALUES (
                    $1,
                    $2,
                    NULL
                )
                ON CONFLICT DO NOTHING;
            "#)
                .bind(to_i(*server_id))
                .bind(to_i(*admin))
                .execute(&mut transaction).await?;
        }
    }
    
    transaction.commit().await?;
    
    // let mut query = String::new();
    // query.push_str(r#"INSERT INTO tags.streams (name, start_time) VALUES"#);
    
    // let mut i = 1usize;
    // for _ in data.streams.iter() {
    //     if i != 1 {
    //         query.push(',');
    //     }
    //     query.push_str(&format!(" (${}, ${})", i, i+1));
    //     i += 2;
    // }
    
    // let mut query = sqlx::query(&query);
    
    // for (stream_name, stream) in data.streams.iter() {
    //     println!("{}", stream_name);
        
    //     query = query.bind(stream_name).bind(stream.start_time);
    //     // sqlx::query(r#"INSERT INTO tags.streams (name, start_time) VALUES ($1, $2)"#)
    //     // // insert_stream
    //     //     .bind(stream_name)
    //     //     .bind(stream.start_time)
    //     //     .execute(&pool).await?;
    // }
    // query.execute(&pool).await?;
    
    Ok(())
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

// mod migrations;
// // async fn migrate() {
// fn migrate() {
//     let url = "-";
    
//     use refinery::config::{ Config, ConfigDbType };
    
//     // let mut config = Config::new(ConfigDbType::Postgres);
    
//     // let mut config = config
//     //     .set_db_user("-")
//     //     .set_db_pass("-")
//     //     .set_db_host("-")
//     //     .set_db_port("5432")
//     //     // .set_db_path(url)
//     //     .set_db_name("-")
//     //     ;
    
//     use std::str::FromStr;
//     let mut config = Config::from_str(url).unwrap();
    
//     // let mut conn = Connection::open_in_memory().unwrap();
    
//     // migrations::runner().run(&mut conn).unwrap();
//     // migrations::runner().run_async(&mut config).await.unwrap();
//     migrations::runner().run(&mut config).unwrap();
// }

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
        Some("test_gentai") => {
            let slug = args.next().unwrap();
            
            let mut rt = Runtime::new().unwrap();
            
            let data = rt.block_on(
                members::gentai::get_members(&slug)
            ).unwrap();
            println!("{:#?}", data);
            
            return
        }
        Some("migrate") => {
            // use discord_lib::tokio::runtime::Runtime;
            let mut rt = Runtime::new().unwrap();
            rt.block_on(migrate()).unwrap();
            // migrate();
            return
        }
        Some("mark_migrated") => {
            let mut rt = Runtime::new().unwrap();
            let version: i64 = args.next().unwrap().parse().unwrap();
            rt.block_on(mark_migrated(version)).unwrap();
            return
        }
        Some("migrate_ron") => {
            // use discord_lib::tokio::runtime::Runtime;
            let mut rt = Runtime::new().unwrap();
            rt.block_on(migrate()).unwrap();
            rt.block_on(migrate_ron()).unwrap();
            // migrate();
            return
        }
        Some("admin") => {
            let op = args.next().unwrap();
            let user: u64 = args.next().unwrap().parse().unwrap();
            
            // use discord_lib::tokio::runtime::Runtime;
            let mut rt = Runtime::new().unwrap();
            rt.block_on(manage_admin(&op, user)).unwrap();
            return
        }
        Some(_) => {
            println!("invalid arg");
            return
        }
        _ => {
            // DB.load_all().unwrap();
        }
    }
    
    // discord_lib::jank_run(discord_stuff());
    // discord_lib::tokio::block_on(discord_stuff());
    // use discord_lib::tokio::runtime::Runtime;
    let mut rt = Runtime::new().unwrap();
    rt.block_on(discord_stuff());
}
