
use sqlx::PgPool;
use std::collections::BTreeSet;

use crate::members::gentai::get_members;
use crate::BulkInsert;
use crate::{ to_i, from_i };
use discord_lib::discord::Snowflake;
use discord_lib::SendHandle;
use anyhow::{ Error, Context, anyhow };
use crate::util::Anyway;

pub struct FullSyncStatus {
    pub errors: usize,
    pub refresh_errors: usize,
    pub roles_errors: usize,
}

pub async fn full_sync(pool: &PgPool, send_handle: &SendHandle) -> Result<FullSyncStatus, Error> {
    let roles: Vec<(i64, String)> = sqlx::query_as(r#"
        SELECT role_id, yt_id
        FROM member.discord_roles, member.yt_channels
        WHERE yt_channel = yt_channels.id
    "#)
        .fetch_all(pool)
        .await.context("get role list")?;
    
    let mut yt_ids = BTreeSet::new();
    let mut role_ids = BTreeSet::new();
    
    for (role_id, yt_id) in roles {
        let role_id = Snowflake(from_i(role_id));
        yt_ids.insert(yt_id);
        role_ids.insert(role_id);
    }
    
    let mut status = FullSyncStatus {
        errors: 0,
        refresh_errors: 0,
        roles_errors: 0,
    };
    
    for yt_id in yt_ids {
        if let Err(err) = refresh_member_list_gentei(pool, send_handle, &yt_id).await.with_context(|| format!("refresh member list {:?}", yt_id)) {
            eprintln!("refresh members error {:?}", err);
            status.refresh_errors += 1;
        }
    }
    for role_id in role_ids {
        if let Err(err) = sync_roles(pool, send_handle, role_id).await.with_context(|| format!("sync roles {:?}", role_id)) {
            eprintln!("sync roles error {:?}", err);
            status.roles_errors += 1;
        }
    }
    status.errors = status.refresh_errors + status.roles_errors;
    
    Ok(status)
}

pub async fn sync_roles(pool: &PgPool, send_handle: &SendHandle, role_id: Snowflake) -> Result<(), Error> {
    let mut transaction = pool.begin().await.context("transaction begin")?;
    // .map_err(|e| {
    //     eprintln!("transaction begin {:?}", e);
    //     HandlerError::with_message("DB error".into())
    // })?;
    
    let (_yt_stream_id, role_id, guild_id, pg_channel_id): (String, i64, i64, i32) = sqlx::query_as(r#"
        SELECT yt_id, role_id, guild_id, yt_channels.id
        FROM member.discord_roles, member.yt_channels
        WHERE
            role_id = $1 and
            discord_roles.yt_channel = yt_channels.id
    "#)
        .bind(to_i(role_id.0))
        .fetch_optional(&mut transaction).await
        .context("find meta")?
        // .map_err(|e| {
        //     eprintln!("find meta {:?}", e);
        //     HandlerError::with_message("DB error".into())
        // })?
        .ok_or_else(|| {
            // HandlerError::with_message("meta not found".into())
            anyhow!("meta not found")
        })?;
    let role_id = from_i(role_id);
    let role = Snowflake(role_id);
    let guild_id = Snowflake(from_i(guild_id));
    
    let known_members: Vec<(i64,)> = sqlx::query_as(r#"
        SELECT discord_id
        FROM member.known_members
        WHERE channel = $1
    "#)
        .bind(pg_channel_id)
        .fetch_all(&mut transaction).await
        .context("find meta")?;
        // .map_err(|e| {
        //     eprintln!("find meta {:?}", e);
        //     HandlerError::with_message("DB error".into())
        // })?;
    
    let blacklist: Vec<(i64,)> = sqlx::query_as(r#"
        SELECT user_id
        FROM member.blacklist, member.discord_roles
        WHERE role_id = $1
    "#)
        .bind(to_i(role_id))
        .fetch_all(&mut transaction).await
        .context("get blacklist")?;
    
    let whitelist: Vec<(i64,)> = sqlx::query_as(r#"
        SELECT user_id
        FROM member.whitelist, member.discord_roles
        WHERE role_id = $1
    "#)
        .bind(to_i(role_id))
        .fetch_all(&mut transaction).await
        .context("get whitelist")?;
    
    let required_roles: Vec<(i64,)> = sqlx::query_as(r#"
        SELECT required_role
        FROM member.required_roles, member.discord_roles
        WHERE role_id = $1
    "#)
        .bind(to_i(role_id))
        .fetch_all(&mut transaction).await
        .context("get required roles")?;
    
    transaction.commit().await.context("transaction commit")?;
    // .map_err(|e| {
    //     eprintln!("transaction commit {:?}", e);
    //     HandlerError::with_message("DB error".into())
    // })?;
    
    let guild_members = send_handle.get_guild_members(guild_id)
        .await.anyway().context("get discord members")?;
        // .map_err(|e| {
        //     eprintln!("get discord members {:?}", e);
        //     HandlerError::with_message("discord error".into())
        // })?;
    
    let has_role = {
        let mut set = BTreeSet::new();
        for member in guild_members.iter() {
            if member.roles.contains(&role) {
                if let Some(ref user) = member.user {
                    set.insert(user.id.0);
                }
            }
        }
        set
    };
    let required_roles: BTreeSet<_> = required_roles.into_iter().map(|(x,)| from_i(x)).collect();
    let has_required_roles = {
        let mut set = BTreeSet::new();
        for member in guild_members.iter() {
            if let Some(ref user) = member.user {
                if required_roles.iter().all(|req| member.roles.contains(&Snowflake(*req))) {
                    set.insert(user.id.0);
                }
            }
        }
        set
    };
    let blacklist = {
        let mut set = BTreeSet::new();
        for (user_id,) in blacklist {
            set.insert(from_i(user_id));
        }
        set
    };
    let whitelist = {
        let mut set = BTreeSet::new();
        for (user_id,) in whitelist {
            set.insert(from_i(user_id));
        }
        set
    };
    let known = {
        let mut set = BTreeSet::new();
        for (member,) in known_members {
            let member = from_i(member);
            set.insert(member);
        }
        set
    };
    let in_server = {
        let mut set = BTreeSet::new();
        for member in guild_members {
            if let Some(user) = member.user {
                set.insert(user.id.0);
            }
        }
        set
    };
    
    let mut add = Vec::new();
    let mut rem = Vec::new();
    
    let is_verified = |user_id: u64| {
        if !in_server.contains(&user_id) {
            return false
        }
        if whitelist.contains(&user_id) {
            return true
        }
        if blacklist.contains(&user_id) {
            return false
        }
        known.contains(&user_id) && has_required_roles.contains(&user_id)
    };
    
    for id in has_role.iter() {
        // if !known.contains(&id) || blacklist.contains(&id) {
        if !is_verified(*id) {
            rem.push(id);
        }
    }
    for id in known.iter() {
        // if !has_role.contains(&id) && in_server.contains(&id) && !blacklist.contains(&id) {
        if !has_role.contains(&id) && is_verified(*id) {
            add.push(id);
        }
    }
    
    // println!("{:#?}", add);
    // println!("{:#?}", rem);
    for user_id in add {
        send_handle.add_member_role(guild_id, Snowflake(*user_id), role)
            .await.anyway().context("add role")?;
            // .map_err(|e| {
            //     eprintln!("add role {:?}", e);
            //     HandlerError::with_message("discord error".into())
            // })?;
    }
    for user_id in rem {
        send_handle.remove_member_role(guild_id, Snowflake(*user_id), role)
            .await.anyway().context("remove_role")?;
            // .with_context(|| format!("remove role"))?;
            // .map_err(|e| {
            //     eprintln!("remove role {:?}", e);
            //     HandlerError::with_message("discord error".into())
            // })?;
    }
    
    Ok(())
}

pub async fn refresh_member_list_gentei(pool: &PgPool, _send_handle: &SendHandle, yt_channel_id: &str) -> Result<(), Error> {
    let mut transaction = pool.begin()
        .await.context("transaction begin")?;
    // .map_err(|e| {
    //     eprintln!("transaction begin {:?}", e);
    //     HandlerError::with_message("DB error".into())
    // })?;
    
    let (channel,): (i32,) = sqlx::query_as(r#"
        SELECT id FROM member.yt_channels
        WHERE yt_id = $1
    "#)
        .bind(yt_channel_id)
        .fetch_optional(&mut transaction)
        .await.context("get channel from yt_id")?
        // .await.map_err(|e| {
        //     eprintln!("get channel from yt_id {:?}", e);
        //     HandlerError::with_message("DB error".into())
        // })?
        .ok_or_else(|| {
            // HandlerError::with_message("channel not found".into())
            anyhow!("channel not found")
        })?;
    
    let (slug,): (String,) = sqlx::query_as(r#"
        SELECT slug FROM member.gentei_slugs
        WHERE channel = $1
    "#)
        .bind(channel)
        .fetch_one(&mut transaction)
        .await.context("get slug for channel")?;
        // .await.map_err(|e| {
        //     eprintln!("get slug for channel {:?}", e);
        //     HandlerError::with_message("DB error".into())
        // })?;
    let ref slug = slug;
    
    let members = get_members(slug).await
    .context("get members")?;
    // println!("{} {:?}", slug, members);
    // .map_err(|e| {
    //     eprintln!("get_members {:?}", e);
    //     HandlerError::with_message("error".into())
    // })?;
    
    sqlx::query(r#"
        DELETE FROM member.known_members
        WHERE channel = $1 and source = 'gentei'
    "#)
        .bind(channel)
        .execute(&mut transaction)
        .await.context("delete members")?;
        // .await.map_err(|e| {
        //     eprintln!("delete members {:?}", e);
        //     HandlerError::with_message("DB error".into())
        // })?;
    
    let mut bulk = BulkInsert::new("member.known_members (discord_id, channel, source)");
    
    #[derive(sqlx::Type)]
    #[sqlx(rename = "known_member_source", rename_all = "lowercase")]
    // #[sqlx(postgres(oid = 16386))]
    #[derive(Debug)]
    enum KnownMemberSource {
        // #[sqlx(rename="gentei")]
        Gentei,
        Badge,
    }
    
    for member in members {
        bulk.row();
        bulk.add(to_i(member.id.0));
        bulk.add(channel);
        // bulk.add("gentei");
        bulk.add(KnownMemberSource::Gentei);
    }
    
    bulk.run(&mut transaction).await.context("run insert")?;
        // .await.map_err(|e| {
        //     eprintln!("run insert {:?}", e);
        //     HandlerError::with_message("DB error".into())
        // })?;
    
    transaction.commit().await.context("transaction commit")?;
    // .map_err(|e| {
    //     eprintln!("transaction commit {:?}", e);
    //     HandlerError::with_message("DB error".into())
    // })?;
    
    Ok(())
}
