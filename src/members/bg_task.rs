
#[allow(unused_imports)] use anyhow::{ Error, Context, anyhow };
use sqlx::PgPool;
use chrono::offset::Utc;
use discord_lib::tokio::time::delay_for;
use std::sync::Arc;

use discord_lib::SendHandle;
use crate::SessionStateDb;
use crate::util::Anyway;
use crate::DateTimeF;

pub(crate) async fn member_task(
    session_db: Arc<SessionStateDb>,
    pool: PgPool,
    send_handle: SendHandle,
    
) -> Result<(), Error> {
    let delay: std::time::Duration = std::time::Duration::new(60, 0);
    
    loop {
        let last_sync = {
            let data = session_db.borrow_data().anyway()?;
            data.last_member_sync.clone()
        };
        
        let now: DateTimeF = Utc::now().into();
        
        if let Some(true) = last_sync.map(|last_sync| 
            (now - last_sync).num_hours() < 24
        ) {
            delay_for(delay).await;
            continue
        }
        
        eprintln!("starting member sync");
        
        let status = crate::members::sync::full_sync(&pool, &send_handle)
            .await.context("full sync")?;
        
        eprintln!("finished member sync errors: {} refresh: {} roles: {}", status.errors, status.refresh_errors, status.roles_errors);
        
        {
            let mut data = session_db.borrow_data_mut().anyway()?;
            data.last_member_sync = Some(now);
        }
        session_db.save().anyway()?;
        
        delay_for(delay).await;
    }
}
