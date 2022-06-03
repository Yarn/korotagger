#![allow(dead_code)]
#![allow(unused_imports)]

use std::collections::BTreeMap;
use std::sync::Arc;

use discord_lib::futures::lock::Mutex;

use anyhow::Result;
use anyhow::anyhow;

use sqlx::PgPool;
use crate::{ from_i, to_i };

#[derive(Debug, Clone)]
pub struct ServerConfig {
    // adjust applied to all streams
    base_adjust: i64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            base_adjust: -20,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServerConfigCache {
    cache: Arc<Mutex<BTreeMap<u64, ServerConfig>>>,
    pool: PgPool,
}

impl ServerConfigCache {
    pub fn new(pool: PgPool) -> Self {
        let cache = Arc::new(Mutex::new(BTreeMap::new()));
        
        Self {
            cache,
            pool,
        }
    }
    
    pub async fn get_config(&mut self, server_id: u64) -> Result<ServerConfig> {
        // self.cache.get(key)
        // wrap in a block to make sure cache is dropped before doing db operations
        {
            let cache = self.cache.lock().await;
            
            if let Some(config) = cache.get(&server_id) {
                return Ok(config.clone());
            }
        }
        
        let config = ServerConfig::default();
        
        let mut cache = self.cache.lock().await;
        cache.insert(server_id, config);
        let config = cache.get(&server_id).expect("server config not in cache after being inserted");
        
        Ok(config.clone())
        
    }
    
    pub async fn save(&mut self, server_id: u64, config: ServerConfig) -> Result<()> {
        let mut cache = self.cache.lock().await;
        
        cache.insert(server_id, config.clone());
        
        Ok(())
    }
}
