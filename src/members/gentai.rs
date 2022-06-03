#![allow(unused_imports)]

use discord_lib::discord::Snowflake;
use serde::{ Deserialize };
use anyhow::{ Context, Error, anyhow };
use crate::util::Anyway;

use discord_lib::hyper::Uri;
use discord_lib::{ hyper, serde_json };
use discord_lib::hyper::body::Body;
use discord_lib::hyper::header::{AUTHORIZATION, CONTENT_TYPE, CONTENT_LENGTH};
use discord_lib::hyper::http::StatusCode;
use discord_lib::hyper::{
    Request,
};

#[derive(Debug, Deserialize)]
pub struct Member {
    pub id: Snowflake,
}

#[derive(Debug, Deserialize)]
struct GentaiResponse {
    #[serde(alias = "user")]
    users: Vec<Member>,
    #[serde(rename= "next", alias = "after")]
    last: Option<Snowflake>,
}

async fn get_members_inner(slug: &str, after: Option<Snowflake>) -> Result<GentaiResponse, Error> {
    let client = discord_lib::send_message::get_client()
        .anyway()
        .with_context(|| "get_client")?;
    
    let auth = "Bearer -";
    
    let mut url = format!(
        "https://us-central1-member-gentei.cloudfunctions.net/\
        API/v1/channel/{}/members?limit=1000",
        slug,
    );
    
    if let Some(Snowflake(after)) = after {
        url.push_str(&format!("&after={}", after))
    }
    
    let req = Request::builder()
        .method("GET")
        .uri(url)
        .header(AUTHORIZATION, auth)
        .header(CONTENT_TYPE, "application/json")
        .header(CONTENT_LENGTH, "0")
        .body(Body::empty())
        // .map_err(|err| err.into())?;
        ?;
    
    // let res = client.get(Uri::from_static("https://api.holotools.app/v1/live"))
    //     // .await.some_error("client get")?;
    //     .await.context("client get")?;
    
    let res = client.request(req).await?;
        
    let (parts, body) = res.into_parts();
    
    if !parts.status.is_success() {
        return Err(anyhow!("Non 200 status code: {:?}", parts.status));
    }
    
    let body = {
        let b: Vec<u8> = hyper::body::to_bytes(body).await?.to_vec();
        
        String::from_utf8(b).context("decode body")?
    };
    
    let data: GentaiResponse = serde_json::from_str(&body)
        .context("parse body")?;
    
    Ok(data)
}

pub async fn get_members(slug: &str) -> Result<Vec<Member>, Error> {
    let mut users = Vec::new();
    
    let mut after = None;
    loop {
        let res = get_members_inner(slug, after)
            .await.context("get_members_inner")?;
        
        for user in res.users {
            users.push(user)
        }
        
        if let None = res.last {
            break;
        }
        after = res.last;
    }
    
    Ok(users)
}
