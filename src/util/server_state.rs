
use std::collections::HashMap;

use serde::{ Serialize, Deserialize };

use discord_lib::discord::{ Snowflake, Guild, Role, Channel, OverwriteType };

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
    guild: Option<Snowflake>,
    everyone_overwrite: Option<SOverwrite>,
    role_overwrites: HashMap<Snowflake, SOverwrite>,
    user_overwrites: HashMap<Snowflake, SOverwrite>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SGuild {
    owner: Snowflake,
    roles: HashMap<Snowflake, SRole>,
    channels: HashMap<Snowflake, SChannel>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerState {
    guilds: HashMap<Snowflake, SGuild>,
}

impl ServerState {
    pub fn new() -> Self {
        Default::default()
    }
    
    pub fn update_guild(&mut self, guild: &Guild) {
        let ref mut state = self.guilds
            .entry(guild.id)
            .or_insert_with(|| {
                SGuild {
                    owner: guild.owner_id,
                    roles: HashMap::new(),
                    channels: HashMap::new(),
                }
            });
        
        state.roles.clear();
        state.channels.clear();
        
        for role in guild.roles.iter() {
            self.update_role(guild.id, role);
        }
        for channel in guild.channels.iter() {
            self.update_channel(channel)
        }
    }
    
    pub fn delete_guild(&mut self, guild: Snowflake) {
        self.guilds.remove(&guild);
    }
    
    pub fn update_channel(&mut self, channel: &Channel) {
        let guild_id = if let Some(guild_id) = channel.guild_id {
            guild_id
        } else { return };
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
            guild: channel.guild_id,
            everyone_overwrite,
            user_overwrites,
            role_overwrites,
        };
        state.channels.insert(channel.id, out);
    }
    
    pub fn delete_channel(&mut self, guild: Snowflake, channel: Snowflake) {
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
    
    pub fn resolve_permissions(&self, guild: Snowflake, channel: Snowflake, user: Snowflake, roles: &[Snowflake]) -> Option<u64> {
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
        let guild = self.guilds.get(&guild_id)?;
        
        let is_owner = user_id == guild.owner;
        
        if is_owner {
            return Some(ALL);
        }
        
        let everyone_role = guild.roles.get(&guild_id)?;
        
        let mut permissions = everyone_role.permissions;
        
        for role_id in roles.iter() {
            let role = guild.roles.get(role_id)?;
            permissions |= role.permissions;
        }
        
        if permissions & ADMINISTRATOR == ADMINISTRATOR {
            return Some(ALL);
        }
        
        let channel = guild.channels.get(&channel_id)?;
        
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
        
        Some(permissions)
    }
}

