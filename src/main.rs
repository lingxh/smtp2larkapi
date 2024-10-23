use chrono::Local;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use serde::{Deserialize, Serialize};
use smtp2larkapi::tools::*;
use smtp2larkapi::{lark_api_mail, smtp_server::*};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Deserialize, Serialize)]
struct Tls {
    cert: String,
    key: String,
}

#[derive(Deserialize, Serialize)]
struct Config {
    user: String,
    passwd: String,
    listener: String,
    host: String,
    safety: String,
    tls: Option<Tls>,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let config_json = read_json("data/config.json")?;
    let config: Config = serde_json::from_value(config_json)?;

    let listener = tokio::net::TcpListener::bind(&config.listener).await?;
    println!("Listening on {}", listener.local_addr()?);

    let mut tls_cert = None;
    if let Some(tls_config) = config.tls {
        let private_key = match PrivateKeyDer::from_pem_file(&tls_config.key) {
            Ok(key) => key,
            Err(e) => panic!("{:?}", e),
        };
        let certs: Vec<_> = CertificateDer::pem_file_iter(&tls_config.cert)
            .unwrap()
            .map(|cert| cert.unwrap())
            .collect();
        tls_cert = Some(Arc::new(
            rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(certs, private_key)?,
        ));
    }

    let mail_config = Arc::new(MailConfig {
        user: config.user.clone(),
        passwd: config.passwd.clone(),
        tls_cert: tls_cert.clone(),
        tls_type: match config.safety.as_str() {
            "starttls" => Some(TlsType::STARTTLS),
            "ssl" => Some(TlsType::SSL),
            _ => None,
        },
        host: config.host.clone(),
    });

    let lark = lark_api_mail::LarkMail::new().await?;
    let lark = Arc::new(RwLock::new(lark));

    loop {
        let lark = lark.clone();
        let mail_config = mail_config.clone();
        let (mut stream, _) = listener.accept().await?;

        tokio::spawn(async move {
            let mut mail = Mail::new(&mut stream, mail_config);
            match mail.run().await {
                Ok(_) => {
                    let Mail { mail_data, .. } = mail;
                    let mail_to = mail_data.to.clone();
                    println!(
                        "{}  Received an email request to send: {:?}",
                        Local::now().format("%Y/%m/%d %H:%M:%S").to_string(),
                        &mail_to
                    );
                    match lark.write().await.send_mail(mail_data).await {
                        Ok(_) => println!(
                            "{}  to: {:?} send success",
                            Local::now().format("%Y/%m/%d %H:%M:%S").to_string(),
                            &mail_to
                        ),
                        Err(e) => println!(
                            "{}  to:{:?}  {}",
                            Local::now().format("%Y/%m/%d %H:%M:%S").to_string(),
                            &mail_to,
                            e.to_string()
                        ),
                    };
                }
                Err(e) => println!(
                    "{} Error: {}",
                    Local::now().format("%Y/%m/%d %H:%M:%S").to_string().trim_end(),
                    e
                ),
            }
        });
    }
}
