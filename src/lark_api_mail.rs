use crate::smtp_server::MailData;
use crate::tools::*;
use anyhow::anyhow;
use base64::engine::general_purpose::URL_SAFE;
use base64::prelude::*;
use chrono::Local;
use mail_parser::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

struct UserToken {
    access_token: String,
    refresh_token: String,
    access_token_expires: u64,
    refresh_token_expires: u64,
}

#[derive(Debug)]
struct AppToken {
    token: String,
    token_expires: u64,
}

#[derive(Clone)]
pub struct AppInfo {
    pub app_id: String,
    pub app_secret: String,
}

pub struct LarkMail {
    app_info: AppInfo,
    app_token: Arc<RwLock<AppToken>>,
    user_token: Arc<RwLock<UserToken>>,
}

#[derive(Deserialize, Serialize)]
struct Attachment {
    body: String,
    filename: String,
}

async fn fetch_app_token(app_info: &AppInfo) -> Result<AppToken, anyhow::Error> {
    let res = reqwest::Client::new()
        .post("https://open.larksuite.com/open-apis/auth/v3/app_access_token/internal")
        .header("Content-Type", "application/json; charset=utf-8")
        .body(format!(
            r#"{{"app_id":"{}","app_secret":"{}"}}"#,
            app_info.app_id, app_info.app_secret
        ))
        .send()
        .await?;

    let json: Value = serde_json::from_str(&res.text().await?)?;

    if json["code"].as_i64() != Some(0) {
        return Err(anyhow!(
            json["msg"]
                .as_str()
                .ok_or(anyhow!(
                    "fetch_app_token: Unable to parse Lark response JSON"
                ))?
                .to_string()
                + "  (fetch_app_token)"
        ));
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    Ok(AppToken {
        token: json["app_access_token"]
            .as_str()
            .ok_or(anyhow!(
                "fetch_app_token: Unable to parse Lark response JSON"
            ))?
            .to_string(),
        token_expires: json["expire"].as_u64().ok_or(anyhow!(""))? + now - 20,
    })
}

async fn fetch_user_token(app_token: &AppToken, code: &str) -> Result<UserToken, anyhow::Error> {
    let error_mag = "fetch_user_token: Unable to parse Lark response JSON";
    let res = reqwest::Client::new()
        .post("https://open.larksuite.com/open-apis/authen/v1/oidc/access_token")
        .header("Content-Type", "application/json; charset=utf-8")
        .header("Authorization", "Bearer ".to_string() + &app_token.token)
        .body(format!(
            r#"{{"grant_type":"authorization_code","code":"{}"}}"#,
            code
        ))
        .send()
        .await?;

    let json: Value = serde_json::from_str(&res.text().await?)?;

    if json["code"].as_i64() != Some(0) {
        return Err(anyhow!(
            json["message"]
                .as_str()
                .ok_or(anyhow!(error_mag))?
                .to_string()
                + "  (fetch_user_token)"
        ));
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    write_json(
        "data/refresh_token.json",
        &serde_json::json!({
            "token" :json["data"]["refresh_token"]
            .as_str()
            .ok_or(anyhow!(error_mag))?
            .to_string(),

            "expires": json["data"]["refresh_expires_in"]
            .as_u64()
            .ok_or(anyhow!(error_mag))?
            + now
            - 20,
        }),
    )?;

    Ok(UserToken {
        access_token: json["data"]["access_token"]
            .as_str()
            .ok_or(anyhow!(error_mag))?
            .to_string(),
        refresh_token: json["data"]["refresh_token"]
            .as_str()
            .ok_or(anyhow!(error_mag))?
            .to_string(),
        access_token_expires: json["data"]["expires_in"].as_u64().ok_or(anyhow!(""))? + now - 20,
        refresh_token_expires: json["data"]["refresh_expires_in"]
            .as_u64()
            .ok_or(anyhow!(error_mag))?
            + now
            - 20,
    })
}

async fn fetch_user_token_refresh(
    app_token: &AppToken,
    user_token: &UserToken,
) -> Result<UserToken, anyhow::Error> {
    let error_mag = "fetch_user_token_refresh: Unable to parse Lark response JSON";
    let res = reqwest::Client::new()
        .post("https://open.larksuite.com/open-apis/authen/v1/oidc/refresh_access_token")
        .header("Content-Type", "application/json; charset=utf-8")
        .header("Authorization", "Bearer ".to_string() + &app_token.token)
        .body(format!(
            r#"{{"grant_type":"refresh_token","refresh_token":"{}"}}"#,
            &user_token.refresh_token
        ))
        .send()
        .await?;

    let json: Value = serde_json::from_str(&res.text().await?)?;

    if json["code"].as_i64() != Some(0) {
        return Err(anyhow!(
            json["message"]
                .as_str()
                .ok_or(anyhow!(error_mag))?
                .to_string()
                + "  (fetch_user_token_refresh)"
        ));
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    write_json(
        "data/refresh_token.json",
        &serde_json::json!({
            "token" :json["data"]["refresh_token"]
            .as_str()
            .ok_or(anyhow!(error_mag))?
            .to_string(),

            "expires": json["data"]["refresh_expires_in"]
            .as_u64()
            .ok_or(anyhow!(error_mag))?
            + now
            - 20,
        }),
    )?;

    Ok(UserToken {
        access_token: json["data"]["access_token"]
            .as_str()
            .ok_or(anyhow!(error_mag))?
            .to_string(),
        refresh_token: json["data"]["refresh_token"]
            .as_str()
            .ok_or(anyhow!(error_mag))?
            .to_string(),
        access_token_expires: json["data"]["expires_in"]
            .as_u64()
            .ok_or(anyhow!(error_mag))?
            + now
            - 20,
        refresh_token_expires: json["data"]["refresh_expires_in"]
            .as_u64()
            .ok_or(anyhow!(error_mag))?
            + now
            - 20,
    })
}

async fn check_token_expires(
    uesr_token: &mut UserToken,
    app_token: &mut AppToken,
    app_info: &AppInfo,
) -> Result<(), anyhow::Error> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    if app_token.token_expires < now {
        let new = fetch_app_token(app_info).await?;
        *app_token = new;
    }

    if uesr_token.access_token_expires < now || uesr_token.refresh_token_expires < now {
        let new = fetch_user_token_refresh(app_token, uesr_token).await?;
        *uesr_token = new;
    }
    Ok(())
}

async fn timing_update(
    uesr_token: Arc<RwLock<UserToken>>,
    app_token: Arc<RwLock<AppToken>>,
    app_info: AppInfo,
) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600 * 24)).await;
        loop {
            use std::result::Result::Ok;
            let mut user_token = uesr_token.write().await;
            let mut app_token = app_token.write().await;
            match check_token_expires(&mut *user_token, &mut *app_token, &app_info).await {
                Ok(_) => break,
                Err(e) => {
                    println!(
                        "{}  {}",
                        Local::now().format("%Y/%m/%d %H:%M:%S").to_string(),
                        e.to_string()
                    );
                    drop(user_token);
                    drop(app_token);
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(600)).await;
        }
    }
}

impl LarkMail {
    pub async fn new() -> Result<Self, anyhow::Error> {
        let error_msg = "Unable to parse json from app_info.json";
        let app_info_config = read_json("data/app_info.json")?;
        let app_info = AppInfo {
            app_id: app_info_config["app_id"]
                .as_str()
                .ok_or(anyhow!(error_msg))?
                .to_string(),
            app_secret: app_info_config["app_secret"]
                .as_str()
                .ok_or(anyhow!(error_msg))?
                .to_string(),
        };

        let code = app_info_config["code"].as_str();

        let app_token = fetch_app_token(&app_info).await?;

        let user_token = if let Some(code) = code {
            write_json(
                "data/app_info.json",
                &json!({
                    "app_id": app_info.app_id,
                    "app_secret": app_info.app_secret,
                }),
            )?;
            fetch_user_token(&app_token, code).await?
        } else {
            let error_mag = "Unable to parse json from refresh_token.json, please re-fill the code at app_info.json to get the token.";
            let json = read_json("data/refresh_token.json")?;
            let mut uesr_token = UserToken {
                access_token: "".to_string(),
                refresh_token: "".to_string(),
                access_token_expires: 0,
                refresh_token_expires: 0,
            };
            uesr_token.refresh_token = json["token"]
                .as_str()
                .ok_or(anyhow!(error_mag))?
                .to_string();
            uesr_token.refresh_token_expires =
                json["expires"].as_u64().ok_or(anyhow!(error_mag))?;
            fetch_user_token_refresh(&app_token, &uesr_token).await?
        };

        let app_token = Arc::new(RwLock::new(app_token));
        let user_token = Arc::new(RwLock::new(user_token));

        let app_token_clone = app_token.clone();
        let user_token_clone = user_token.clone();
        let app_info_clone = app_info.clone();
        tokio::spawn(async move {
            timing_update(user_token_clone, app_token_clone, app_info_clone).await;
        });

        Ok(LarkMail {
            app_info: app_info,
            user_token: user_token.clone(),
            app_token: app_token.clone(),
        })
    }

    pub async fn send_mail(&mut self, mail_data: MailData) -> Result<(), anyhow::Error> {
        let user_token = &mut *self.user_token.write().await;
        let app_token = &mut *self.app_token.write().await;

        let message = MessageParser::default()
            .parse(&mail_data.body)
            .ok_or(anyhow!("Failed to parse the email content"))?;

        let mut name = String::new();
        if let Some(from) = message.from() {
            if let Some(from) = from.first() {
                name = from.name().unwrap_or("").to_string();
            }
        }

        let mut json = serde_json::json!({
            "subject": message.subject().unwrap_or(""),
            "to": mail_data.to,
            "head_from" : json!({
                "name": name
            }),
        });

        let mut html = String::new();
        let mut attachments = Vec::new();

        if let Some(body_html) = message.body_html(0) {
            html = body_html.to_string();
        };

        for attachment in message.attachments() {
            if attachment.content_disposition().is_some()
                && attachment.content_disposition().unwrap().ctype() == "inline"
            {
                if attachment.content_type().is_none() {
                    continue;
                }
                let content_type = attachment.content_type().unwrap();
                if content_type.c_subtype.is_none() {
                    continue;
                }
                let ctype = format!(
                    "{}/{}",
                    content_type.c_type.to_string(),
                    content_type.c_subtype.as_ref().unwrap().to_string()
                );
                if attachment.content_id().is_none() {
                    continue;
                }
                html = html.replace(
                    &format!("cid:{}", attachment.content_id().unwrap()),
                    &format!(
                        "data:{};base64,{}",
                        ctype,
                        BASE64_STANDARD.encode(attachment.contents())
                    ),
                )
            } else {
                let filename = attachment.attachment_name().unwrap_or("未知");
                attachments.push(Attachment {
                    body: URL_SAFE.encode(attachment.contents()),
                    filename: filename.to_string(),
                });
            }
        }

        if html.len() != 0 {
            json["body_html"] = html.into();
        }

        if attachments.len() != 0 {
            json["attachments"] = serde_json::to_value(&attachments)?;
        }

        check_token_expires(user_token, app_token, &self.app_info).await?;
        let res = reqwest::Client::new()
            .post("https://open.larksuite.com/open-apis/mail/v1/user_mailboxes/me/messages/send")
            .header("Content-Type", "application/json; charset=utf-8")
            .header(
                "Authorization",
                "Bearer ".to_string() + &user_token.access_token,
            )
            .body(json.to_string())
            .send()
            .await?;

        let json: Value = serde_json::from_str(&res.text().await?)?;
        let error_msg = "send_mail: Unable to parse Lark response JSON";
        if json["code"].as_i64() != Some(0) {
            return Err(anyhow!(json["msg"]
                .as_str()
                .ok_or(anyhow!(error_msg))?
                .to_string()));
        }
        Ok(())
    }
}
