
use async_trait::async_trait;
use sqlx::PgPool;

use crate::handlers::{ Handler, HandlerResult, HandlerError, Command };
use crate::members::gentai::get_members;
use crate::{ to_i, from_i };
use discord_lib::discord::Snowflake;
use discord_lib::send_message::NewMessage;

#[derive(Debug)]
pub struct VerifyHandler {
    pub pool: PgPool,
}

#[async_trait]
impl Handler for VerifyHandler {
    async fn handle_command_b(&self, command: Command<'_>) -> HandlerResult {
        let ref server_id = command.message.guild_id.as_ref()
            .ok_or(HandlerError::with_message("Not in a server".into()))?;
        
        let roles: Vec<(i64,)> = sqlx::query_as(r#"
            SELECT role_id
            FROM member.discord_roles
            WHERE guild_id = $1
        "#)
            .bind(to_i(server_id.0))
            .fetch_all(&self.pool)
            .await.map_err(|e| {
                eprintln!("get roles {:?}", e);
                HandlerError::with_message("DB error".into())
            })?;
        
        let mut errors: usize = 0;
        for role_id in roles.iter() {
            if let Err(err) = crate::members::sync::
                sync_roles(&self.pool, &command.send_handle, Snowflake(from_i(role_id.0))).await
            {
                eprintln!("role sync {:?}", err);
                errors += 1;
            }
        }
        
        if errors > 0 {
            Ok(format!("partial failure {}/{} failed", errors, roles.len()).into())
        } else {
            Ok("_".into())
        }
        
        // panic!();
        // Ok("_".into())
    }
}

#[derive(Debug)]
pub struct MembersHandler {
    pub pool: PgPool,
}

async fn wblist_cmd(pool: &PgPool, command: &Command<'_>, mode: &str) -> HandlerResult {
    command.require_admin().await?;
    
    let cmd = command.args.get(2)
        .ok_or(HandlerError::with_message("Param required".into()))?;
    
    let ref guild_id = command.message.guild_id.as_ref()
        .ok_or(HandlerError::with_message("Not in a server".into()))?;
    
    match *cmd {
        "list" => {
            // let ref guild_id = command.message.guild_id.as_ref()
            //     .ok_or(HandlerError::with_message("Not in a server".into()))?;
            
            let users: Vec<(i64,)> = sqlx::query_as(&format!(r#"
                SELECT user_id
                FROM member.{}, member.discord_roles
                WHERE discord_roles.id = role and guild_id = $1
            "#, mode))
                // .bind(to_i(role_id))
                .bind(to_i(guild_id.0))
                // .bind(channel)
                // .execute(&mut transaction)
                .fetch_all(pool)
                .await.map_err(|e| {
                    eprintln!("list {} {:?}", mode, e);
                    HandlerError::with_message("DB error".into())
                })?;
            
            let mut out = String::new();
            
            for (user_id,) in users {
                // out.push_str(&format!("<@&{}>\n", from_i(role_id)));
                out.push_str(&format!("<@{}>\n", from_i(user_id)));
            }
            
            let mut msg: NewMessage = out.into();
            msg.suppress_mentions();
            
            return Ok(msg.into());
        }
        "add" => {
            let help_err = || HandlerError::with_message(format!("{} add <role_id> <user_id>", mode));
            
            let role_id: u64 = command.args.get(3)
                // .ok_or(HandlerError::with_message(USAGE_STR.into()))?
                .ok_or_else(help_err)?
                .parse().map_err(|_| HandlerError::with_message("failed to parse <role_id>".into()))?;
            let user_id: u64 = command.args.get(4)
                // .ok_or(HandlerError::with_message(USAGE_STR.into()))?
                .ok_or_else(help_err)?
                .parse().map_err(|_| HandlerError::with_message("failed to parse <user_id>".into()))?;
            
            sqlx::query(&format!(r#"
                INSERT INTO member.{} (role, user_id)
                VALUES (
                    (SELECT id FROM member.discord_roles WHERE role_id = $2 and guild_id = $3),
                    $1
                )
            "#, mode))
                .bind(to_i(user_id))
                .bind(to_i(role_id))
                .bind(to_i(guild_id.0))
                // .bind(channel)
                .execute(pool)
                .await.map_err(|e| {
                    eprintln!("insert {} {:?}", mode, e);
                    HandlerError::with_message("DB error".into())
                })?;
        }
        "rem" => {
            let help_err = || HandlerError::with_message(format!("{} rem <role_id> <user_id>", mode));
            
            let role_id: u64 = command.args.get(3)
                // .ok_or(HandlerError::with_message(USAGE_STR.into()))?
                .ok_or_else(help_err)?
                .parse().map_err(|_| HandlerError::with_message("failed to parse <role_id>".into()))?;
            let user_id: u64 = command.args.get(4)
                // .ok_or(HandlerError::with_message(USAGE_STR.into()))?
                .ok_or_else(help_err)?
                .parse().map_err(|_| HandlerError::with_message("failed to parse <user_id>".into()))?;
            
            sqlx::query(&format!(r#"
                --INSERT INTO member. (role, user_id)
                DELETE FROM member.{} USING member.discord_roles
                WHERE role = discord_roles.id and role_id = $2 and guild_id = $3 and user_id = $1
                --VALUES (
                --    (SELECT id FROM member.discord_roles WHERE role_id = $2 and guild_id = $3),
                --    $1
                --)
            "#, mode))
                .bind(to_i(user_id))
                .bind(to_i(role_id))
                .bind(to_i(guild_id.0))
                // .bind(channel)
                .execute(pool)
                .await.map_err(|e| {
                    eprintln!("insert {} {:?}", mode, e);
                    HandlerError::with_message("DB error".into())
                })?;
        }
        _ => {
            return Ok(format!("{} <list/add/rem>", mode).into())
        }
    }
    
    return Ok("_".into())
}

#[async_trait]
impl Handler for MembersHandler {
    async fn handle_command_b(&self, command: Command<'_>) -> HandlerResult {
        let cmd = command.args.get(0)
            .ok_or(HandlerError::with_message("Param required".into()))?;
        
        let res = match *cmd {
            "list_roles" => {
                command.require_admin().await?;
                
                let ref guild_id = command.message.guild_id.as_ref()
                    .ok_or(HandlerError::with_message("Not in a server".into()))?;
                
                let roles: Vec<(i64,)> = sqlx::query_as(r#"
                    SELECT role_id
                    FROM member.discord_roles
                    WHERE guild_id = $1
                    --       (role_id, guild_id, yt_channel)
                    --VALUES ($1,      $2,       $3)
                    --ON CONFLICT (role_id)
                    --    DO UPDATE SET guild_id = $2, yt_channel = $3
                "#)
                    // .bind(to_i(role_id))
                    .bind(to_i(guild_id.0))
                    // .bind(channel)
                    // .execute(&mut transaction)
                    .fetch_all(&self.pool)
                    .await.map_err(|e| {
                        eprintln!("list discord role {:?}", e);
                        HandlerError::with_message("DB error".into())
                    })?;
                
                let mut out = String::new();
                
                for (role_id,) in roles {
                    out.push_str(&format!("<@&{}>\n", from_i(role_id)));
                }
                
                let mut msg: NewMessage = out.into();
                msg.suppress_mentions();
                
                return Ok(msg.into());
            }
            "add_role" => {
                command.require_admin().await?;
                
                const USAGE_STR: &str = "Usage: add_role <yt_channel_id> <role_id>";
                
                let ref guild_id = command.message.guild_id.as_ref()
                    .ok_or(HandlerError::with_message("Not in a server".into()))?;
                let yt_channel_id = command.args.get(1)
                    .ok_or(HandlerError::with_message(USAGE_STR.into()))?;
                let role_id: u64 = command.args.get(2)
                    .ok_or(HandlerError::with_message(USAGE_STR.into()))?
                    .parse().map_err(|_| HandlerError::with_message("could not parse role_id".into()))?;
                
                let mut transaction = self.pool.begin().await.map_err(|e| {
                    eprintln!("transaction begin {:?}", e);
                    HandlerError::with_message("DB error".into())
                })?;
                
                let (channel,): (i32,) = sqlx::query_as(r#"
                    SELECT id FROM member.yt_channels
                    WHERE yt_id = $1
                "#)
                    .bind(yt_channel_id)
                    .fetch_optional(&mut transaction)
                    .await.map_err(|e| {
                        eprintln!("get channel from yt_id {:?}", e);
                        HandlerError::with_message("DB error".into())
                    })?
                    .ok_or_else(|| {
                        HandlerError::with_message("yt channel not found".into())
                    })?;
                
                sqlx::query(r#"
                    INSERT INTO member.discord_roles
                           (role_id, guild_id, yt_channel)
                    VALUES ($1,      $2,       $3)
                    --ON CONFLICT (role_id)
                    --    DO UPDATE SET guild_id = $2, yt_channel = $3
                "#)
                    .bind(to_i(role_id))
                    .bind(to_i(guild_id.0))
                    .bind(channel)
                    .execute(&mut transaction)
                    .await.map_err(|e| {
                        eprintln!("insert discord role {:?}", e);
                        HandlerError::with_message("DB error".into())
                    })?;
                
                transaction.commit().await.map_err(|e| {
                    eprintln!("transaction commit {:?}", e);
                    HandlerError::with_message("DB error".into())
                })?;
                
                "_"
            }
            "rem_role" => {
                command.require_admin().await?;
                
                let ref guild_id = command.message.guild_id.as_ref()
                    .ok_or(HandlerError::with_message("Not in a server".into()))?;
                let role_id: u64 = command.args.get(2)
                    .ok_or(HandlerError::with_message("Usage: rem_role <role_id>".into()))?
                    .parse().map_err(|_| HandlerError::with_message("could not parse role_id".into()))?;
                
                // let mut transaction = self.pool.begin().await.map_err(|e| {
                //     eprintln!("transaction begin {:?}", e);
                //     HandlerError::with_message("DB error".into())
                // })?;
                
                // let (channel,): (i32,) = sqlx::query_as(r#"
                //     SELECT id FROM member.yt_channels
                //     WHERE yt_id = $1
                // "#)
                //     .bind(yt_channel_id)
                //     .fetch_optional(&mut transaction)
                //     .await.map_err(|e| {
                //         eprintln!("get channel from yt_id {:?}", e);
                //         HandlerError::with_message("DB error".into())
                //     })?
                //     .ok_or_else(|| {
                //         HandlerError::with_message("yt channel not found".into())
                //     })?;
                
                sqlx::query(r#"
                    DELETE FROM member.discord_roles
                    WHERE role_id = $1 and guild_id = $2
                    --       (role_id, guild_id, yt_channel)
                    --VALUES ($1,      $2,       $3)
                    --ON CONFLICT (role_id)
                    --    DO UPDATE SET guild_id = $2, yt_channel = $3
                "#)
                    .bind(to_i(role_id))
                    .bind(to_i(guild_id.0))
                    // .bind(channel)
                    // .execute(&mut transaction)
                    .execute(&self.pool)
                    .await.map_err(|e| {
                        eprintln!("delete discord role {:?}", e);
                        HandlerError::with_message("DB error".into())
                    })?;
                
                // transaction.commit().await.map_err(|e| {
                //     eprintln!("transaction commit {:?}", e);
                //     HandlerError::with_message("DB error".into())
                // })?;
                
                "_"
            }
            "list_req_role" => {
                command.require_admin().await?;
                
                let ref guild_id = command.message.guild_id.as_ref()
                    .ok_or(HandlerError::with_message("Not in a server".into()))?;
                
                let required_roles: Vec<(i64, i64)> = sqlx::query_as(r#"
                    SELECT role_id, required_role
                    FROM member.required_roles, member.discord_roles
                    WHERE role = discord_roles.id and guild_id = $1
                "#)
                    .bind(to_i(guild_id.0))
                    .fetch_all(&self.pool)
                    .await.map_err(|e| {
                        eprintln!("list required role {:?}", e);
                        HandlerError::with_message("DB error".into())
                    })?;
                
                let mut out = String::new();
                
                for (role_id, required_role_id) in required_roles {
                    out.push_str(&format!("<@&{}> <@&{}>\n", from_i(role_id), from_i(required_role_id)));
                }
                
                let mut msg: NewMessage = out.into();
                msg.suppress_mentions();
                
                return Ok(msg.into());
            }
            "add_req_role" => {
                command.require_admin().await?;
                
                const USAGE_STR: &str = "Usage: add_req_role <member_role> <required_role>";
                
                let ref guild_id = command.message.guild_id.as_ref()
                    .ok_or(HandlerError::with_message("Not in a server".into()))?;
                let member_role = command.args.get(1)
                    .ok_or(HandlerError::with_message(USAGE_STR.into()))?
                    .parse().map_err(|_| HandlerError::with_message("could not parse member_role".into()))?;
                let required_role: u64 = command.args.get(2)
                    .ok_or(HandlerError::with_message(USAGE_STR.into()))?
                    .parse().map_err(|_| HandlerError::with_message("could not parse required_role".into()))?;
                
                sqlx::query(r#"
                    INSERT INTO member.required_roles (role, required_role)
                    VALUES (
                        (SELECT id FROM member.discord_roles WHERE role_id = $2 and guild_id = $3),
                        $1
                    )
                "#)
                    .bind(to_i(required_role))
                    .bind(to_i(member_role))
                    .bind(to_i(guild_id.0))
                    // .bind(channel)
                    // .execute(&mut transaction)
                    .execute(&self.pool)
                    .await.map_err(|e| {
                        eprintln!("insert required role {:?}", e);
                        HandlerError::with_message("DB error".into())
                    })?;
                
                "_"
            }
            "rem_req_role" => {
                command.require_admin().await?;
                
                const USAGE_STR: &str = "Usage: rem_req_role <member_role> <required_role>";
                
                let ref guild_id = command.message.guild_id.as_ref()
                    .ok_or(HandlerError::with_message("Not in a server".into()))?;
                let member_role = command.args.get(1)
                    .ok_or(HandlerError::with_message(USAGE_STR.into()))?
                    .parse().map_err(|_| HandlerError::with_message("could not parse member_role".into()))?;
                let required_role: u64 = command.args.get(2)
                    .ok_or(HandlerError::with_message(USAGE_STR.into()))?
                    .parse().map_err(|_| HandlerError::with_message("could not parse required_role".into()))?;
                
                sqlx::query(r#"
                    DELET FROM member.required_roles USING member.discord_roles
                    WHERE role = discord_roles.id and required_role = $1 and role_id = $2 and guild_id = $3
                "#)
                    .bind(to_i(required_role))
                    .bind(to_i(member_role))
                    .bind(to_i(guild_id.0))
                    // .bind(channel)
                    // .execute(&mut transaction)
                    .execute(&self.pool)
                    .await.map_err(|e| {
                        eprintln!("remove required role {:?}", e);
                        HandlerError::with_message("DB error".into())
                    })?;
                
                "_"
            }
            "whitelist" => {
                return wblist_cmd(&self.pool, &command, "whilelist").await;
            }
            "blacklist" => {
                return wblist_cmd(&self.pool, &command, "blacklist").await;
            }
            _ => {
                "unknown command"
            }
        };
        
        Ok(res.into())
    }
}

#[derive(Debug)]
pub struct MembersAdminHandler {
    pub pool: PgPool,
}

#[async_trait]
impl Handler for MembersAdminHandler {
    async fn handle_command_b(&self, command: Command<'_>) -> HandlerResult {
        // command.require_global_admin().await?;
        
        let cmd = command.args.get(0)
            .ok_or(HandlerError::with_message("Param required".into()))?;
        
        let res = match *cmd {
            "x" => {
                let slug = command.args.get(1)
                    .ok_or(HandlerError::with_message("Usage: x <slug>".into()))?;
                
                println!("x");
                
                let members = get_members(slug).await.map_err(|e| {
                    eprintln!("get_members {:?}", e);
                    HandlerError::with_message("error".into())
                })?;
                
                println!("{:#?}", members);
                
                "x"
            }
            "set_channel" => {
                let yt_channel_id = command.args.get(1)
                    .ok_or(HandlerError::with_message("Usage: set_channel <yt_channel_id> <readable?>".into()))?;
                let readable: Option<&str> = command.args.get(2).map(|x| *x);
                
                sqlx::query(r#"
                    INSERT INTO member.yt_channels (yt_id, readable)
                    VALUES ($1, $2)
                    ON CONFLICT (yt_id)
                        DO UPDATE SET readable = $2
                "#)
                    .bind(yt_channel_id)
                    .bind(readable)
                    .execute(&self.pool)
                    .await.map_err(|e| {
                        eprintln!("insert slug {:?}", e);
                        HandlerError::with_message("DB error".into())
                    })?;
                
                "_"
            }
            "set_gentei" => {
                let yt_channel_id = command.args.get(1)
                    .ok_or(HandlerError::with_message("Usage: set_gentei <yt_channel_id> <slug>".into()))?;
                let slug = command.args.get(2)
                    .ok_or(HandlerError::with_message("Usage: set_gentei <yt_channel_id> <slug>".into()))?;
                
                let mut transaction = self.pool.begin().await.map_err(|e| {
                    eprintln!("transaction begin {:?}", e);
                    HandlerError::with_message("DB error".into())
                })?;
                
                let (channel,): (i32,) = sqlx::query_as(r#"
                    SELECT id FROM member.yt_channels
                    WHERE yt_id = $1
                "#)
                    .bind(yt_channel_id)
                    .fetch_optional(&mut transaction)
                    .await.map_err(|e| {
                        eprintln!("get channel from yt_id {:?}", e);
                        HandlerError::with_message("DB error".into())
                    })?
                    .ok_or_else(|| {
                        HandlerError::with_message("channel not found".into())
                    })?;
                
                sqlx::query(r#"
                    INSERT INTO member.gentei_slugs (channel, slug)
                    VALUES ($1, $2)
                    ON CONFLICT (channel)
                        DO UPDATE SET slug = $2
                "#)
                    .bind(channel)
                    .bind(slug)
                    .execute(&mut transaction)
                    .await.map_err(|e| {
                        eprintln!("insert slug {:?}", e);
                        HandlerError::with_message("DB error".into())
                    })?;
                
                transaction.commit().await.map_err(|e| {
                    eprintln!("transaction commit {:?}", e);
                    HandlerError::with_message("DB error".into())
                })?;
                
                "_"
            }
            "set_role" => {
                let yt_channel_id = command.args.get(1)
                    .ok_or(HandlerError::with_message("Usage: set_role <yt_channel_id> <role_id> <guild_id>".into()))?;
                let role_id: u64 = command.args.get(2)
                    .ok_or(HandlerError::with_message("Usage: set_role <yt_channel_id> <role_id> <guild_id>".into()))?
                    .parse().map_err(|_| HandlerError::with_message("could not parse role_id".into()))?;
                let guild_id: u64 = command.args.get(3)
                    .ok_or(HandlerError::with_message("Usage: set_role <yt_channel_id> <role_id> <guild_id>".into()))?
                    .parse().map_err(|_| HandlerError::with_message("could not parse guild_id".into()))?;
                
                let mut transaction = self.pool.begin().await.map_err(|e| {
                    eprintln!("transaction begin {:?}", e);
                    HandlerError::with_message("DB error".into())
                })?;
                
                let (channel,): (i32,) = sqlx::query_as(r#"
                    SELECT id FROM member.yt_channels
                    WHERE yt_id = $1
                "#)
                    .bind(yt_channel_id)
                    .fetch_optional(&mut transaction)
                    .await.map_err(|e| {
                        eprintln!("get channel from yt_id {:?}", e);
                        HandlerError::with_message("DB error".into())
                    })?
                    .ok_or_else(|| {
                        HandlerError::with_message("yt channel not found".into())
                    })?;
                
                sqlx::query(r#"
                    INSERT INTO member.discord_roles
                           (role_id, guild_id, yt_channel)
                    VALUES ($1,      $2,       $3)
                    ON CONFLICT (role_id)
                        DO UPDATE SET guild_id = $2, yt_channel = $3
                "#)
                    .bind(to_i(role_id))
                    .bind(to_i(guild_id))
                    .bind(channel)
                    .execute(&mut transaction)
                    .await.map_err(|e| {
                        eprintln!("insert discord role {:?}", e);
                        HandlerError::with_message("DB error".into())
                    })?;
                
                transaction.commit().await.map_err(|e| {
                    eprintln!("transaction commit {:?}", e);
                    HandlerError::with_message("DB error".into())
                })?;
                
                "_"
            }
            "refresh_gentei" => {
                let yt_channel_id = command.args.get(1)
                    .ok_or(HandlerError::with_message("Usage: refresh <yt_channel_id>".into()))?;
                
                crate::members::sync::refresh_member_list_gentei(&self.pool, &command.send_handle, &yt_channel_id)
                    .await.map_err(|e| {
                        eprintln!("refresh member list {:?}", e);
                        HandlerError::with_message("error".into())
                    })?;
                
                "_"
            }
            "sync_roles" => {
                // let guild_id = command.message.guild_id.as_ref()
                //     .ok_or(HandlerError::with_message("Not in a server".into()))?;
                let role_id: u64 = command.args.get(1)
                    .ok_or(HandlerError::with_message("Usage: sync_roles <role_id>".into()))?
                    .parse().map_err(|_| HandlerError::with_message("could not parse role_id".into()))?;
                
                crate::members::sync::sync_roles(&self.pool, &command.send_handle, Snowflake(role_id))
                    .await.map_err(|e| {
                        eprintln!("members sync {:?}", e);
                        HandlerError::with_message("error".into())
                    })?;
                
                "_"
            }
            "sync" => {
                let status = crate::members::sync::full_sync(&self.pool, &command.send_handle)
                    .await.map_err(|e| {
                        eprintln!("full sync {:?}", e);
                        HandlerError::with_message("error".into())
                    })?;
                
                return Ok(format!("errors: {} refresh: {} roles: {}", status.errors, status.refresh_errors, status.roles_errors).into())
            }
            _ => {
                "unknown command"
            }
        };
        
        // panic!()
        Ok(res.into())
    }
}
