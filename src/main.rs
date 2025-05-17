
use discord_lib::discord;
use rustbreak::FileDatabase;
use rustbreak::deser::Ron;
#[allow(unused_imports)]
use std::collections::{ HashMap, HashSet, BTreeMap, BTreeSet };
use serde::{Serialize, Deserialize};
use chrono::{ DateTime, FixedOffset };
use tokio::time::sleep;
use tokio;
use tokio::sync::RwLock;
use tokio::runtime::Runtime;

mod auto_stream_live;
mod i_love_youtube;
mod permissions;
mod message_formatting;
mod message_handling;
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

use std::sync::Arc;
use discord_lib::SendHandle;

use sqlx::PgPool;

use util::server_state::ServerState;
use handlers::{ Handler, HandlerResult };

type DateTimeF = DateTime<FixedOffset>;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SharedState {
    #[serde(default)]
    active: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    #[serde(default)]
    clean_shutdown: bool,
    session_id: Option<String>,
    seq: Option<u64>,
    self_id: Option<u64>,
    #[serde(default)]
    server_state: ServerState,
}

impl SessionState {
    fn new() -> Self {
        SessionState {
            clean_shutdown: false,
            session_id: None,
            seq: None,
            self_id: None,
            server_state: Default::default(),
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
    name: Arc<String>,
    send_handle: Arc<SendHandle>,
    session: Arc<FileDatabase<SessionState, Ron>>,
    #[allow(dead_code)]
    shared: Arc<FileDatabase<SharedState, Ron>>,
    pool: PgPool,
    handlers: Arc<BTreeMap<&'static str, Box<dyn Handler>>>,
    command_aliases: Arc<BTreeMap<&'static str, &'static str>>,
    
    server_state: Arc<RwLock<ServerState>>,
}

impl DiscordState {
    pub fn new(
        name: String,
        send_handle: SendHandle,
        session: Arc<FileDatabase<SessionState, Ron>>,
        shared: Arc<FileDatabase<SharedState, Ron>>,
        pool: PgPool,
        handlers: Arc<BTreeMap<&'static str, Box<dyn Handler>>>,
        command_aliases: Arc<BTreeMap<&'static str, &'static str>>,
    ) -> Self {
        Self {
            name: Arc::new(name),
            send_handle: Arc::new(send_handle),
            session,
            shared,
            pool,
            handlers,
            command_aliases,
            server_state: Default::default(),
        }
    }
    
    pub async fn new_session(&self) {
        let mut server_state = self.server_state.write().await;
        *server_state = Default::default();
    }
    
    async fn save_server_state(&self) {
        let server_state = Arc::clone(&self.server_state);
        let session = Arc::clone(&self.session);
        tokio::task::spawn_blocking(move || {
            let state = server_state.blocking_read();
            let mut data = session.borrow_data_mut().unwrap();
            data.server_state = state.clone();
            std::mem::drop(state);
            std::mem::drop(data);
            session.save().unwrap();
        }).await.unwrap();
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
        pool: PgPool,
        handlers: Arc<BTreeMap<&'static str, Box<dyn Handler>>>,
        command_aliases: Arc<BTreeMap<&'static str, &'static str>>,
    ) -> Self {
        
        
        let mut last_seq: Option<u64> = None;
        let discord_obj = {
            let mut data = session_state.borrow_data_mut().unwrap();
            let discord = match (&data.session_id, &data.seq, &data.clean_shutdown) {
                (Some(ref session_id), Some(seq), true) => {
                    last_seq = data.seq;
                    
                    discord_lib::Discord::reconnect_with(BASE_URL.into(), token.to_string(), session_id.clone(), *seq).await.unwrap()
                }
                _ => {
                    data.session_id = None;
                    data.seq = None;
                    discord_lib::Discord::connect(BASE_URL.into(), token.to_string()).await.unwrap()
                }
            };
            data.clean_shutdown = false;
            discord
        };
        
        let send_handle = discord_obj.get_send_handle();
        let d_state = DiscordState::new(
            name.to_string(),
            send_handle.clone(),
            session_state.clone(),
            shared_state.clone(),
            pool,
            handlers,
            command_aliases,
        );
        
        let bot = Bot {
            discord: discord_obj,
            state: d_state,
            seq: last_seq,
        };
        
        bot
    }
}

const BASE_URL: &'static str = "https://discord.com/api/v10";

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
    
    let handlers = {
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
        Arc::new(handlers)
    };
    
    let mut command_aliases = BTreeMap::new();
    command_aliases.insert("t", "tag");
    let command_aliases = Arc::new(command_aliases);
    
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
            pool.clone(),
            handlers.clone(),
            command_aliases.clone(),
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
        
        loop {
            let task = auto_live::holodex::auto_live_task(
                &mut active, &task_pool,
                states.as_slice(),
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
    
    async fn handle_discord_connection(
        mut bot: Bot,
        mut stopping: tokio::sync::mpsc::Receiver<()>,
    ) -> Result<Option<u64>, ()> {
        let mut self_id = {
            let data = bot.state.session.borrow_data_mut().unwrap();
            data.self_id.map(|x| discord::Snowflake(x))
        };
        let ref mut discord_obj = bot.discord;
        let ref discord_state = bot.state;
        let name = discord_state.name.as_str();
        let ref session_state = discord_state.session;
        
        let shared_state = Arc::new(discord_state.clone());
        
        loop {
            use discord_lib::gateway::GatewayError;
            
            let res = tokio::select! {
                msg = discord_obj.recv() => {
                    msg
                }
                _ = stopping.recv() => {
                    return Ok(bot.seq);
                }
            };
            
            match res {
                Ok(msg) => {
                    use discord_lib::gateway::GatewayMessage as GM;
                    use discord_lib::gateway::Event as E;
                    
                    match msg {
                        GM::Event(E::Ready(ready)) => {
                            {
                                let mut data = session_state.borrow_data_mut().unwrap();
                                data.session_id = Some(ready.session_id.clone());
                                data.self_id = Some(ready.user.id.0);
                            }
                            self_id = Some(ready.user.id);
                            shared_state.save_server_state().await;
                            eprintln!("{:>10}: ready", name);
                        }
                        GM::Event(E::Resumed) => {
                            let mut server_state = discord_state.server_state.write().await;
                            {
                                let data = session_state.borrow_data().unwrap();
                                *server_state = data.server_state.clone();
                            }
                            std::mem::drop(server_state);
                            eprintln!("{:>10}: resumed", name);
                        }
                        GM::Hello(_) => {}
                        // GM::Event(E::Unknown(event_type, _)) if event_type.as_str() == "MESSAGE_REACTION_REMOVE_ALL".to_string() => {}
                        // GM::Event(E::Unknown(event_type, _)) if event_type.as_str() == "MESSAGE_DELETE_BULK".to_string() => {}
                        // GM::Event(E::Unknown(event_type, _)) if event_type.as_str() == "GUILD_MEMBER_ADD".to_string() => {}
                        // GM::Event(E::Unknown(event_type, _)) if event_type.as_str() == "GUILD_MEMBER_REMOVE".to_string() => {}
                        // GM::Event(E::Unknown(event_type, _)) if event_type.as_str() == "GUILD_MEMBER_UPDATE".to_string() => {}
                        // GM::Event(E::Unknown(event_type, _)) if event_type.as_str() == "GUILD_BAN_ADD".to_string() => {}
                        // GM::Event(E::Unknown(event_type, _)) if event_type.as_str() == "GUILD_EMOJIS_UPDATE".to_string() => {}
                        // GM::Event(E::Unknown(event_type, _)) if event_type.as_str() == "GUILD_BAN_REMOVE".to_string() => {}
                        // GM::Event(E::Unknown(event_type, _)) if event_type.as_str() == "TYPING_START".to_string() => {}
                        // GM::Event(E::Unknown(event_type, _)) if event_type.as_str() == "CHANNEL_PINS_UPDATE".to_string() => {}
                        // GM::Event(E::Unknown(event_type, _)) if event_type.as_str() == "INVITE_CREATE".to_string() => {}
                        // GM::Event(E::Unknown(event_type, _)) if event_type.as_str() == "INVITE_DELETE".to_string() => {}
                        // GM::Event(E::Unknown(event_type, _)) if event_type.as_str() == "GUILD_JOIN_REQUEST_DELETE".to_string() => {}
                        // GM::Event(E::Unknown(event_type, _)) if event_type.as_str() == "INTEGRATION_CREATE".to_string() => {}
                        // GM::Event(E::Unknown(event_type, _)) if event_type.as_str() == "WEBHOOKS_UPDATE".to_string() => {}
                        GM::Event(event) => {
                            message_handling::handle_event(
                                event,
                                shared_state.clone(),
                                self_id,
                            ).await;
                        }
                        
                        GM::Raw(_msg) => {
                            // eprintln!("\n\n?RAW {:?}", _msg);
                        }
                        _other => {
                            // eprintln!("\n\n?OTHER {:?}", _other);
                        }
                    }
                },
                Err(err) => {
                    eprintln!("Err recieving message: {:?}", err);
                    
                    if let GatewayError::InvalidSession = err {
                        eprintln!("InvalidSession: ");
                        discord_state.new_session().await;
                        bot.seq = None;
                        {
                            let mut data = session_state.borrow_data_mut().unwrap();
                            
                            data.session_id = None;
                            data.seq = None;
                        }
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
                }
            }
            // dbg!(&msg);
            
            bot.seq = discord_obj.seq();
        }
    }
    
    let mut tasks = Vec::new();
    for bot in bots.into_iter() {
        let (stop_send, stopping) = tokio::sync::mpsc::channel(1);
        let state = bot.state.clone();
        let task = tokio::task::spawn(handle_discord_connection(
            bot,
            stopping,
        ));
        tasks.push((task, state, stop_send));
    }
    
    tokio::signal::ctrl_c().await.unwrap();
    eprintln!("stopping");
    let mut all_stopped = async move || {
        for (_, _, stop_send) in tasks.iter_mut() {
            let _ = stop_send.try_send(());
        }
        
        for (task, state, _) in tasks.iter_mut() {
            match task.await {
                Ok(Ok(seq)) => {
                    {
                        let mut data = state.session.borrow_data_mut().unwrap();
                        data.seq = seq;
                        data.clean_shutdown = true;
                    }
                    state.save_server_state().await;
                    eprintln!(". {}", state.name);
                },
                err => {
                    let _ = dbg!(err);
                }
            };
            
        }
    };
    let res = tokio::time::timeout(std::time::Duration::from_secs(2), all_stopped()).await;
    match res {
        Ok(_) => {
            eprintln!("stopped");
        },
        Err(_) => {
            eprintln!("stopping timed out");
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
        }
        Some("migrate") => {
            let rt = Runtime::new().unwrap();
            rt.block_on(migrate()).unwrap();
        }
        Some("mark_migrated") => {
            let rt = Runtime::new().unwrap();
            let version: i64 = args.next().unwrap().parse().unwrap();
            rt.block_on(mark_migrated(version)).unwrap();
        }
        Some("admin") => {
            let op = args.next().unwrap();
            let user: u64 = args.next().unwrap().parse().unwrap();
            
            let rt = Runtime::new().unwrap();
            rt.block_on(manage_admin(&op, user)).unwrap();
        }
        Some("from_i") => {
            let val = args.next().unwrap();
            let val: i64 = val.parse().unwrap();
            println!("{}", from_i(val));
        }
        Some("to_i") => {
            let val = args.next().unwrap();
            let val: u64 = val.parse().unwrap();
            println!("{}", to_i(val));
        }
        Some(_) => {
            println!("invalid arg");
        }
        None => {
            let rt = Runtime::new().unwrap();
            rt.block_on(discord_stuff());
        }
    }
}
