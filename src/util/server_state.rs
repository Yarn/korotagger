
use std::collections::{ HashMap, HashSet };
use std::sync::Arc;
use tokio::sync::RwLock;
use miette::miette;

use serde::{ Serialize, Deserialize };

use discord_lib::discord::{ Snowflake, Guild, Role, Channel, OverwriteType };
use discord_lib::SendHandle;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SRole {
    id: Snowflake,
    guild: Snowflake,
    permissions: u64,
    order: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SOverwrite {
    id: Snowflake,
    allow: u64,
    deny: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SChannel {
    everyone_overwrite: Option<SOverwrite>,
    role_overwrites: HashMap<Snowflake, SOverwrite>,
    user_overwrites: HashMap<Snowflake, SOverwrite>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SGuild {
    owner: Option<Snowflake>,
    roles: HashMap<Snowflake, SRole>,
    channels: HashMap<Snowflake, SChannel>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerState {
    guilds: HashMap<Snowflake, SGuild>,
    #[serde(default)]
    unavailable_channels: HashSet<Snowflake>,
    #[serde(skip)]
    pub(crate) send_handle: Option<Arc<SendHandle>>,
}

impl ServerState {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Default::default()
    }
    
    pub fn clear(&mut self) {
        self.guilds.clear();
        self.unavailable_channels.clear();
    }
    
    pub async fn get_guild_of_channel(locked: &RwLock<Self>, channel_id: Snowflake) -> miette::Result<Snowflake> {
        let state = locked.read().await;
        
        if state.unavailable_channels.contains(&channel_id) {
            return Err(miette!("get_channel marked invalid {:?}", channel_id));
        }
        
        let res = state.get_cached_guild_of_channel(channel_id);
        if let Some(guild_id) = res {
            return Ok(guild_id);
        }
        let send_handle = state.send_handle.as_ref()
            .ok_or_else(|| miette!("send_handle not set when looking up guild for channel {:?}", channel_id))?.clone();
        std::mem::drop(state);
        
        let channel = match send_handle.get_channel(channel_id).await {
            Ok(channel) => channel,
            Err(err @ discord_lib::send_message::Error::DiscordError{status_code, ..}) => {
                let mut state = locked.write().await;
                // see note on same code below
                let res = state.get_cached_guild_of_channel(channel_id);
                if let Some(guild_id) = res {
                    return Ok(guild_id);
                }
                match status_code.as_u16() {
                    403 | 404 => {
                        state.unavailable_channels.insert(channel_id);
                        return Err(miette!("get_channel error (marking invalid) {:?}", err));
                    }
                    _ => {
                        return Err(miette!("get_channel error {:?}", err));
                    }
                }
                
            }
            Err(err) => {
                return Err(miette!("get_channel error {:?}", err));
            }
        };
        
        let guild_id = channel.guild_id.ok_or_else(|| miette!("channel not part of guild"))?;
        
        let channel = discord_lib::discord::Channel {
            id: channel.id,
            guild_id: None,
            permission_overwrites: channel.permission_overwrites,
        };
        
        let mut state = locked.write().await;
        
        // check to make sure an update_channel hasn't happened while making the api call
        // this can presumably still be reached while an update_channel is in transit that reflects
        // a newer state than the api call
        let res = state.get_cached_guild_of_channel(channel_id);
        if let Some(guild_id) = res {
            return Ok(guild_id);
        }
        
        state.update_channel_with_guild(guild_id, &channel);
        std::mem::drop(state);
        
        Ok(guild_id)
    }
    
    fn get_cached_guild_of_channel(&self, channel_id: Snowflake) -> Option<Snowflake> {
        for (guild_id, guild) in self.guilds.iter() {
            if let Some(_channel) = guild.channels.get(&channel_id) {
                return Some(*guild_id);
            }
        }
        None
    }
    
    pub fn mark_guild_exists(&mut self, guild_id: Snowflake) {
        self.guilds.entry(guild_id).or_insert_with(|| {
            SGuild {
                owner: None,
                roles: HashMap::new(),
                channels: HashMap::new(),
            }
        });
    }
    
    pub fn update_guild(&mut self, guild: &Guild) {
        let state = self.guilds
            .entry(guild.id)
            .or_insert_with(|| {
                SGuild {
                    owner: None,
                    roles: HashMap::new(),
                    channels: HashMap::new(),
                }
            });
        
        state.owner = Some(guild.owner_id);
        state.roles.clear();
        
        for role in guild.roles.iter() {
            self.update_role(guild.id, role);
        }
        for channel in guild.channels.iter() {
            self.update_channel_with_guild(guild.id, &channel)
        }
    }
    
    pub fn delete_guild(&mut self, guild: Snowflake) {
        self.guilds.remove(&guild);
    }
    
    fn update_channel_with_guild(&mut self, guild_id: Snowflake, channel: &Channel) {
        let state = if let Some(state) = self.guilds.get_mut(&guild_id) {
            state
        } else { return };
        
        let mut everyone_overwrite = None;
        let mut user_overwrites = HashMap::new();
        let mut role_overwrites = HashMap::new();
        if let Some(in_overwrites) = channel.permission_overwrites.as_ref() {
            for o in in_overwrites {
                let overwrite = SOverwrite {
                    id: o.id,
                    allow: o.allow,
                    deny: o.deny,
                };
                match o.overwrite_type {
                    OverwriteType::Role => {
                        if overwrite.id == guild_id {
                            everyone_overwrite = Some(overwrite);
                        } else {
                            role_overwrites.insert(overwrite.id, overwrite);
                        }
                    }
                    OverwriteType::Member => {
                        user_overwrites.insert(overwrite.id, overwrite);
                    }
                    _ => ()
                }
            }
        }
        
        let out = SChannel {
            everyone_overwrite,
            user_overwrites,
            role_overwrites,
        };
        state.channels.insert(channel.id, out);
    }
    
    pub fn update_channel(&mut self, channel: &Channel) {
        self.unavailable_channels.remove(&channel.id);
        let guild_id = if let Some(guild_id) = channel.guild_id {
            guild_id
        } else { return };
        
        self.update_channel_with_guild(guild_id, channel)
    }
    
    pub fn delete_channel(&mut self, guild: Snowflake, channel: Snowflake) {
        self.unavailable_channels.remove(&channel);
        let state = if let Some(state) = self.guilds.get_mut(&guild) {
            state
        } else { return };
        state.channels.remove(&channel);
    }
    
    pub fn update_role(&mut self, guild: Snowflake, role: &Role) {
        let state = if let Some(state) = self.guilds.get_mut(&guild) {
            state
        } else { return };
        
        let out = SRole {
            id: role.id,
            guild,
            permissions: role.permissions,
            order: role.position,
        };
        
        state.roles.insert(out.id, out);
    }
    
    pub fn delete_role(&mut self, guild: Snowflake, role: Snowflake) {
        let state = if let Some(state) = self.guilds.get_mut(&guild) {
            state
        } else { return };
        
        state.roles.remove(&role);
    }
    
    pub fn resolve_permissions(&self, guild: Snowflake, channel: Snowflake, user: Snowflake, roles: &[Snowflake]) -> Result<u64, String> {
        const ADMINISTRATOR: u64 = 0x0000000000000008;
        const ALL: u64 = {
            let mut acc = 0;
            let mut i = 0;
            while i <= 46 {
                acc += 1<<i;
                i+=1;
            }
            acc += 1<<49;
            acc += 1<<50;
            acc
        };
        
        let guild_id = guild;
        let channel_id = channel;
        let user_id = user;
        let guild = self.guilds.get(&guild_id).ok_or_else(|| format!("guild not found {guild_id:?}"))?;
        
        let is_owner = Some(user_id) == guild.owner;
        
        if is_owner {
            return Ok(ALL);
        }
        
        let everyone_role = guild.roles.get(&guild_id).ok_or_else(|| format!("everyone role not found {guild_id:?}"))?;
        
        let mut permissions = everyone_role.permissions;
        
        for role_id in roles.iter() {
            let role = guild.roles.get(role_id).ok_or_else(|| format!("role not found {role_id:?} in guild {guild_id:?}"))?;
            permissions |= role.permissions;
        }
        
        if permissions & ADMINISTRATOR == ADMINISTRATOR {
            return Ok(ALL);
        }
        
        if let Some(channel) = guild.channels.get(&channel_id) {
            if let Some(ref ov) = channel.everyone_overwrite {
                permissions &= !ov.deny;
                permissions |= ov.allow;
            }
            
            let mut allow = 0;
            let mut deny = 0;
            for role_id in roles.iter() {
                if let Some(overwrite) = channel.role_overwrites.get(role_id) {
                    allow |= overwrite.allow;
                    deny |= overwrite.deny;
                }
            }
            
            permissions &= !deny;
            permissions |= allow;
            
            if let Some(overwrite) = channel.user_overwrites.get(&user_id) {
                permissions &= !overwrite.deny;
                permissions |= overwrite.allow;
            }
        }
        
        Ok(permissions)
    }
}

