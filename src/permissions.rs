
use sqlx::prelude::Executor;
use sqlx::Postgres;
use discord_lib::discord::{
    Snowflake,
    Message,
};
use crate::to_i;
use anyhow::{Context, Error};

pub async fn is_msg_from_admin<'c, E>(exec: E, msg: &Message) -> Result<bool, ()>
    where
        E: Executor<'c, Database = Postgres>
{
    // let data = DB.config_state.borrow_data().unwrap();
    
    let guild_id = msg.guild_id.as_ref().map(|x| to_i(x.0));
    let user_id = to_i(msg.author.id.0);
    
    let mut role_query_part = String::new();
    // let mut role_query_part_server = String::new();
    if let Some(ref member) = msg.member {
        let ref roles = member.roles;
        for Snowflake(role_id) in roles {
            // role_query_part.push_str(&format!(
            //     r#"admins."group" = {0} or"#,
            //     to_i(*role_id)
            // ));
            role_query_part.push_str(&format!(
                r#"server_admins."group" = {0} or "#,
                to_i(*role_id)
            ));
        }
    }
    if role_query_part == "" {
        role_query_part.push_str("false");
    }
    // let role_query_part = role_query_part.strip_suffix(" or").unwrap();
    let role_query_part = role_query_part.strip_suffix(" or ").unwrap_or(&role_query_part);
    
    let query = format!(r#"
        SELECT true
        FROM config.admins
        FULL JOIN config.server_admins ON false
        WHERE
            (admins."group" = $2) or
            (({} or server_admins."group" = $2) and server = $1)
    "#, role_query_part);
    // eprintln!("{}", query);
    // eprintln!("{:?} {:?}", guild_id, user_id);
    
    let admin: Option<(bool,)> = sqlx::query_as(&query)
        .bind(guild_id)
        .bind(user_id)
        .fetch_optional(exec).await.map_err(|e| {
            eprintln!("check admin {:?}", e);
        })?;
    
    Ok(admin.is_some())
    
    // let guild_perms = match msg.guild_id {
    //     Some(Snowflake(guild_id_raw)) => {
    //         match data.admin_perm.get(&guild_id_raw) {
    //             Some(guild_perms) => guild_perms,
    //             None => return Ok(false)
    //         }
    //     }
    //     None => return Ok(false)
    // };
    
    // let Snowflake(user_id_raw) = msg.author.id;
    
    // if guild_perms.contains(&user_id_raw) {
    //     return Ok(true)
    // }
    
    // if let Some(ref member) = msg.member {
    //     let ref roles = member.roles;
    //     for Snowflake(role_id) in roles {
    //         if guild_perms.contains(&role_id) {
    //             return Ok(true)
    //         }
    //     }
    // }
    
    // let is_admin = data.admin_perm
    //     .get(&guild_id_raw)
    //     .map(|admins| admins.contains(&user_id_raw))
    //     .unwrap_or(false);
    
    // Ok(is_admin)
    // Ok(false)
}

pub async fn is_msg_from_global_admin<'c, E>(exec: E, msg: &Message) -> Result<bool, Error>
    where
        E: Executor<'c, Database = Postgres>
{
    let user_id = to_i(msg.author.id.0);
    
    let query = r#"
        SELECT true
        FROM config.admins
        WHERE
            admins."group" = $1
    "#;
    
    let admin: Option<(bool,)> = sqlx::query_as(&query)
        .bind(user_id)
        .fetch_optional(exec)
        .await.context("global admin query")?;
    
    Ok(admin.is_some())
}
