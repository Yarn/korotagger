
use rustbreak::FileDatabase;
use rustbreak::deser::Ron;
#[allow(unused_imports)]
use std::collections::{ HashMap, HashSet, BTreeMap, BTreeSet };
use std::path::PathBuf;
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
use std::sync::atomic::{ Ordering, AtomicBool };
use discord_lib::{ Discord, SendHandle };
use discord_lib::discord::Snowflake;

use sqlx::PgPool;

use util::server_state::ServerState;
use util::Stoppable;
use handlers::{ Handler, HandlerResult };

type DateTimeF = DateTime<FixedOffset>;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SharedState {
    #[serde(default)]
    active: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Session {
    self_id: Snowflake,
    session_id: String,
    seq: u64,
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
    session_path: Arc<PathBuf>,
    session_lock: Arc<RwLock<bool>>,
    pool: PgPool,
    handlers: Arc<BTreeMap<&'static str, Box<dyn Handler>>>,
    command_aliases: Arc<BTreeMap<&'static str, &'static str>>,
    is_ready: Arc<AtomicBool>,
    
    server_state: Arc<RwLock<ServerState>>,
}

impl DiscordState {
    pub fn new(
        name: String,
        send_handle: SendHandle,
        session_path: Arc<PathBuf>,
        pool: PgPool,
        handlers: Arc<BTreeMap<&'static str, Box<dyn Handler>>>,
        command_aliases: Arc<BTreeMap<&'static str, &'static str>>,
    ) -> Self {
        let send_handle = Arc::new(send_handle);
        let mut server_state = ServerState::new();
        server_state.send_handle = Some(Arc::clone(&send_handle));
        
        Self {
            name: Arc::new(name),
            send_handle: send_handle,
            session_path,
            session_lock: Default::default(),
            pool,
            handlers,
            command_aliases,
            is_ready: Arc::new(AtomicBool::new(false)),
            server_state: Arc::new(RwLock::new(server_state)),
        }
    }
    
    async fn load_session(&self) -> miette::Result<Option<Session>> {
        let server_state = Arc::clone(&self.server_state);
        let session_lock = Arc::clone(&self.session_lock);
        let session_path = Arc::clone(&self.session_path);
        let send_handle = self.send_handle.clone();
        tokio::task::spawn_blocking(move || {
            let lock = session_lock.blocking_read();
            
            let file = match std::fs::File::open(&*session_path) {
                Ok(file) => file,
                Err(err) => {
                    match err.kind() {
                        std::io::ErrorKind::NotFound => {
                            return Ok(None);
                        }
                        _ => ()
                    }
                    return Err(miette::Report::from_err(err));
                }
            };
            use miette::IntoDiagnostic as _;
            
            #[derive(Deserialize)]
            struct SessionState {
                session: Option<Session>,
                server_state: ServerState,
            }
            
            let state: SessionState = serde_json::from_reader(file).into_diagnostic()?;
            
            let mut server_state = server_state.blocking_write();
            *server_state = state.server_state;
            server_state.send_handle = Some(send_handle.clone());
            
            std::mem::drop(server_state);
            std::mem::drop(lock);
            Ok(state.session)
        }).await.unwrap()
        
    }
    
    async fn save_session(&self, session: Option<Session>) {
        let server_state = Arc::clone(&self.server_state);
        let session_lock = Arc::clone(&self.session_lock);
        let session_path = Arc::clone(&self.session_path);
        tokio::task::spawn_blocking(move || {
            let lock = session_lock.blocking_write();
            
            #[derive(Serialize)]
            struct SessionState<'a> {
                session: Option<Session>,
                server_state: &'a ServerState,
            }
            
            let state = server_state.blocking_read();
            let session = SessionState {
                session: session,
                server_state: &state,
            };
            let file = std::fs::File::create(&*session_path).unwrap();
            serde_json::to_writer(file, &session).unwrap();
            
            std::mem::drop(state);
            std::mem::drop(lock);
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
}

impl Bot {
    async fn new(
        session_path: Arc<PathBuf>,
        name: &str,
        token: &str,
        pool: PgPool,
        handlers: Arc<BTreeMap<&'static str, Box<dyn Handler>>>,
        command_aliases: Arc<BTreeMap<&'static str, &'static str>>,
    ) -> Self {
        let discord_obj = discord_lib::Discord::new(BASE_URL.into(), token.to_string());
        
        let send_handle = discord_obj.get_send_handle();
        let d_state = DiscordState::new(
            name.to_string(),
            send_handle,
            session_path,
            pool,
            handlers,
            command_aliases,
        );
        
        let bot = Bot {
            discord: discord_obj,
            state: d_state,
        };
        
        bot
    }
}

async fn connect(discord_obj: &mut Discord, session: &Option<Session>, stoppable: &mut Stoppable) -> Result<(), ()> {
    let mut wait_secs = 1;
    loop {
        let inner = async {
            match session.as_ref() {
                Some(session) => {
                    discord_obj.reconnect_with(
                        session.session_id.clone(),
                        session.seq,
                    ).await
                }
                None => {
                    discord_obj.connect().await
                }
            }
        };
        
        match stoppable.run(inner).await {
            Some(Ok(())) => {
                break
            }
            Some(Err(err)) => {
                println!("Could not establish connection ({}s): {:?}", wait_secs, err);
                stoppable.run(sleep(::std::time::Duration::new(wait_secs, 0))).await
                    .ok_or(())?;
                if wait_secs < 20 {
                    wait_secs *= 2;
                }
            }
            None => return Err(()),
        }
    };
    Ok(())
}

async fn reconnect(discord_obj: &mut Discord, stoppable: &mut Stoppable) -> Result<(), ()> {
    let mut wait_secs = 1;
    loop {
        match stoppable.run(discord_obj.reconnect()).await {
            Some(Ok(is_reconnect)) => {
                if is_reconnect {
                    eprintln!("IS RECONNECT");
                }
                break
            }
            Some(Err(err)) => {
                println!("Could not re-establish connection ({}s): {:?}", wait_secs, err);
                stoppable.run(sleep(::std::time::Duration::new(wait_secs, 0))).await
                    .ok_or(())?;
                if wait_secs < 20 {
                    wait_secs *= 2;
                }
            }
            None => return Err(()),
        }
    };
    Ok(())
}

async fn handle_discord_connection(
    mut bot: Bot,
    mut stoppable: Stoppable,
) -> miette::Result<Session> {
    let ref mut discord_obj = bot.discord;
    let ref discord_state = bot.state;
    let name = discord_state.name.as_str();
    
    let discord_state = Arc::new(discord_state.clone());
    
    use discord_lib::gateway::GatewayMessage as GM;
    use discord_lib::gateway::Event as E;
    
    let session = discord_state.load_session().await?;
    
    match session.as_ref() {
        Some(session) => {
            eprintln!("{:>10}: reconnecting {:?}", bot.state.name, session);
        }
        None => {}
    }
    if let Err(()) = connect(discord_obj, &session, &mut stoppable).await {
        return session.ok_or_else(|| miette::miette!("no session"));
    }
    
    let mut old_session: miette::Result<Session> = session.ok_or_else(|| miette::miette!("no session"));
    loop {
        let mut session = loop {
            let res = match stoppable.run(discord_obj.recv()).await {
                Some(res) => res,
                None => return old_session,
            };
            
            match res {
                Ok(GM::Event(E::Ready(ready))) => {
                    {
                        let mut state = discord_state.server_state.write().await;
                        state.clear();
                        for guild in ready.guilds {
                            state.mark_guild_exists(guild.id);
                        }
                        state.send_handle = Some(discord_state.send_handle.clone());
                    }
                    
                    discord_state.is_ready.store(true, Ordering::Relaxed);
                    eprintln!("{:>10}: ready", name);
                    break Session {
                        self_id: ready.user.id,
                        session_id: ready.session_id,
                        seq: discord_obj.seq().unwrap_or(0),
                    };
                }
                Ok(GM::Event(E::Resumed)) => {
                    discord_state.is_ready.store(true, Ordering::Relaxed);
                    eprintln!("{:>10}: resumed", name);
                    break old_session.unwrap();
                }
                Ok(_) => {}
                Err(err) => {
                    eprintln!("Err recieving message: {:?}", err);
                    if let Err(()) = reconnect(discord_obj, &mut stoppable).await {
                        return old_session;
                    }
                }
            }
        };
        
        loop {
            let res = match stoppable.run(discord_obj.recv()).await {
                Some(res) => res,
                None => return Ok(session),
            };
            
            match res {
                Ok(msg) => {
                    match msg {
                        GM::Hello(_) => {}
                        
                        GM::Event(event) => {
                            message_handling::handle_event(
                                event,
                                discord_state.clone(),
                                Some(session.self_id),
                            ).await;
                        }
                        
                        GM::Raw(_msg) => {
                            // eprintln!("\n\n?RAW {:?}", _msg);
                        }
                        _other => {
                            // eprintln!("\n\n?OTHER {:?}", _other);
                        }
                    }
                    session.seq = discord_obj.seq().unwrap_or(0);
                },
                Err(err) => {
                    eprintln!("Err recieving message: {:?}", err);
                    if let Err(()) = reconnect(discord_obj, &mut stoppable).await {
                        return Ok(session);
                    }
                    break;
                }
            }
        }
        old_session = Ok(session);
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
        
        let mut new_path = state_path.clone();
        new_path.push(&format!("kq_session_{}.json", &config.name));
        
        let bot = Bot::new(
            Arc::new(new_path),
            &config.name,
            &config.token,
            pool.clone(),
            handlers.clone(),
            command_aliases.clone(),
        ).await;
        eprintln!("{:>10}:", bot.state.name);
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
        
        for state in states.iter() {
            if !state.is_ready.load(Ordering::Relaxed) {
                sleep(::std::time::Duration::from_millis(100)).await;
            }
        }
        dbg!("auto live starting");
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
            
            sleep(::std::time::Duration::new(30, 0)).await;
        }
    });
    
    let mut tasks = Vec::new();
    for bot in bots.into_iter() {
        let (stopper, stoppable) = Stoppable::new();
        let state = bot.state.clone();
        let task = tokio::task::spawn(handle_discord_connection(
            bot,
            stoppable,
        ));
        tasks.push((task, state, stopper));
    }
    
    tokio::signal::ctrl_c().await.unwrap();
    eprintln!("stopping");
    let mut all_stopped = async move || {
        for (_, _, stopper) in tasks.iter_mut() {
            let _ = stopper.stop().await;
        }
        
        for (task, state, _) in tasks.iter_mut() {
            match task.await {
                Ok(Ok(session)) => {
                    state.save_session(
                        Some(session),
                    ).await;
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
