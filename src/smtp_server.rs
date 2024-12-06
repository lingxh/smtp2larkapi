use anyhow::anyhow;
use base64::prelude::*;
use serde::{Deserialize, Serialize};
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::RwLock;
use tokio::time::timeout;
use tokio_rustls::{rustls, TlsAcceptor};
pub struct Mail<S>
where
    S: AsyncReadExt + AsyncWriteExt + Sync + Send + Unpin,
{
    pub mail_data: MailData,
    host: String,
    user: String,
    passwd: String,
    stream: Arc<RwLock<S>>,
    status: Status,
    tls_type: Option<TlsType>,
    tls_cert: Option<Arc<rustls::ServerConfig>>,
    auth_type: String,
}

#[derive(Debug)]
pub struct MailData {
    pub from: Addr,
    pub to: Vec<Addr>,
    pub subject: String,
    pub body: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Addr {
    pub mail_address: String,
    pub name: String,
}

pub struct MailConfig {
    pub user: String,
    pub passwd: String,
    pub host: String,
    pub default_name: String,
    pub tls_type: Option<TlsType>,
    pub tls_cert: Option<Arc<rustls::ServerConfig>>,
}

#[derive(PartialEq, Clone)]
pub enum TlsType {
    STARTTLS,
    SSL,
}

struct Status {
    has_tls: bool,
    auth: bool,
    quit: bool,
    starttls: bool,
    auth_login_begin: bool,
    lock: LockMode,
}

#[derive(PartialEq)]
enum LockMode {
    NULL,
    DATA,
    AUTH,
}
pub fn plain_encode(user: &str, password: &str) -> String {
    BASE64_STANDARD.encode(format!("\x00{}\x00{}", user, password))
}

impl<S> Mail<S>
where
    S: AsyncReadExt + AsyncWriteExt + Sync + Send + Unpin,
{
    pub fn new(stream: S, config: Arc<MailConfig>) -> Self {
        Mail {
            mail_data: MailData {
                from: Addr {
                    mail_address: "".to_string(),
                    name: config.default_name.clone(),
                },
                to: Vec::new(),
                subject: String::new(),
                body: String::new(),
            },
            host: config.host.clone(),
            user: config.user.clone(),
            passwd: config.passwd.clone(),
            stream: Arc::new(RwLock::new(stream)),
            status: Status {
                has_tls: false,
                auth: false,
                quit: false,
                starttls: false,
                auth_login_begin: false,
                lock: LockMode::NULL,
            },
            tls_cert: config.tls_cert.clone(),
            tls_type: config.tls_type.clone(),
            auth_type: "".to_string(),
        }
    }
}

impl<S> Mail<S>
where
    S: AsyncReadExt + AsyncWriteExt + Sync + Send + Unpin,
{
    pub async fn run(&mut self) -> Result<(), anyhow::Error> {
        for i in 0..2 {
            if self.tls_type.is_some() && *self.tls_type.as_ref().unwrap() == TlsType::SSL && i == 0
            {
                self.status.has_tls = true;
                continue;
            }
            let stream = self.stream.clone();
            let mut stream = stream.write().await;
            if let Some(tls_cert) = &self.tls_cert {
                if self.status.has_tls {
                    let conn = TlsAcceptor::from(tls_cert.clone())
                        .accept(&mut *stream)
                        .await?;
                    self.io(BufReader::new(conn)).await?;
                } else {
                    self.io(BufReader::new(&mut *stream)).await?;
                }
            } else {
                self.io(BufReader::new(&mut *stream)).await?;
            };

            if !self.status.has_tls {
                break;
            }
        }
        Ok(())
    }

    fn check_mail(&self) -> bool {
        self.mail_data.to.len() != 0
            && self.mail_data.from.mail_address.len() != 0
            && self.mail_data.body.len() != 0
    }

    async fn io<IO>(&mut self, mut reader: IO) -> Result<(), anyhow::Error>
    where
        IO: AsyncBufReadExt + AsyncWriteExt + Sync + Send + Unpin,
    {
        if !self.status.has_tls
            || self.tls_type.is_some() && *self.tls_type.as_ref().unwrap() == TlsType::SSL
        {
            reader
                .write_all(format!("220 {} Esmtp smtp2larkapi\r\n", &self.host).as_bytes())
                .await?;
        }
        loop {
            let mut request = String::new();
            match timeout(Duration::from_secs(10), reader.read_line(&mut request)).await? {
                Ok(_) => {
                    if cfg!(debug_assertions) {
                        print!("receive:  {}", &request);
                    }
                }
                Err(e) => {
                    if e.kind() == io::ErrorKind::UnexpectedEof {
                        return Ok(());
                    }
                    return Err(e.into());
                }
            }

            match self.scheduler(&request).await {
                Ok(response) => {
                    if response.len() != 0 {
                        if cfg!(debug_assertions) {
                            print!("send:  {}", response);
                        }

                        reader.write_all(response.as_bytes()).await?;
                        if self.status.quit {
                            return Ok(());
                        }
                        if self.status.starttls {
                            self.status.starttls = false;
                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    if self.check_mail() {
                        return Ok(());
                    }
                    if cfg!(debug_assertions) {
                        print!("send:  {}", e.to_string());
                    }
                    reader.write_all(e.to_string().as_bytes()).await?;
                    return Err(e);
                }
            }
        }
    }

    async fn scheduler(&mut self, request: &str) -> Result<String, anyhow::Error> {
        let handle = match self.status.lock {
            LockMode::NULL => request
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_uppercase(),
            LockMode::DATA => "DATA".to_string(),
            LockMode::AUTH => "AUTH".to_string(),
        };

        let response: Result<String, anyhow::Error> = match handle.as_str() {
            "HELO" | "EHLO" => self.helo().await,
            "STARTTLS" => self.starttls().await,
            "MAIL" => self.mail(&request).await,
            "RCPT" => self.rcpt(&request).await,
            "DATA" => self.data(&request).await,
            "QUIT" => self.quit().await,
            "AUTH" => self.auth(&request).await,
            _ => Err(anyhow!("500 Unknown command")),
        };

        response
    }

    async fn helo(&self) -> Result<String, anyhow::Error> {
        let tls = if self.tls_type.is_some()
            && *self.tls_type.as_ref().unwrap() == TlsType::STARTTLS
            && !self.status.has_tls
        {
            "250-STARTTLS\r\n"
        } else {
            ""
        };
        Ok(format!("250-{}\r\n250-PIPELINING\r\n250-SIZE 73400320\r\n{}250-AUTH LOGIN PLAIN\r\n250-AUTH=LOGIN\r\n250-SMTPUTF8\r\n250 8BITMIME\r\n", &self.host, tls))
    }

    async fn mail(&mut self, request: &str) -> Result<String, anyhow::Error> {
        if !self.status.auth {
            return Err(anyhow!("Client is not authenticated"));
        }
        let left_index = request
            .find("<")
            .ok_or(anyhow!("500 Unable to parse content\r\n"))?
            + 1;
        let right_index = request
            .find(">")
            .ok_or(anyhow!("500 Unable to parse content\r\n"))?;
        if right_index - left_index < 1 {
            return Err(anyhow!("500"));
        }
        let from = &request[left_index..right_index];
        self.mail_data.from.mail_address = from.to_string();

        Ok("250 OK\r\n".to_string())
    }

    async fn rcpt(&mut self, request: &str) -> Result<String, anyhow::Error> {
        if !self.status.auth {
            return Err(anyhow!("Client is not authenticated"));
        }
        let left_index = request
            .find("<")
            .ok_or(anyhow!("500 Unable to parse content\r\n"))?
            + 1;
        let right_index = request
            .find(">")
            .ok_or(anyhow!("500 Unable to parse content\r\n"))?;
        if right_index - left_index < 1 {
            return Err(anyhow!("500"));
        }
        let to = &request[left_index..right_index];
        self.mail_data.to.push(Addr {
            mail_address: to.to_string(),
            name: "".to_string(),
        });

        Ok("250 OK\r\n".to_string())
    }

    async fn data(&mut self, request: &str) -> Result<String, anyhow::Error> {
        if !self.status.auth {
            return Err(anyhow!("Client is not authenticated"));
        }
        if request == ".\r\n" {
            self.status.lock = LockMode::NULL;
            return Ok("250 OK\r\n".to_string());
        }

        if self.status.lock == LockMode::DATA {
            if request == "..\r\n" {
                self.mail_data.body += ".\r\n"
            } else {
                self.mail_data.body += request;
            }

            return Ok(String::new());
        }

        self.status.lock = LockMode::DATA;
        Ok("354 Start mail input; end with <CRLF>.<CRLF>\r\n".to_string())
    }

    async fn quit(&mut self) -> Result<String, anyhow::Error> {
        self.status.quit = true;
        Ok("221 Bye\r\n".to_string())
    }

    async fn auth(&mut self, request: &str) -> Result<String, anyhow::Error> {
        if self.tls_type.is_some() && !self.status.has_tls {
            return Err(anyhow!("530 5.7.0 Must issue a STARTTLS command first\r\n"));
        }

        let args = if self.status.lock == LockMode::NULL {
            let args = request.split(" ").collect::<Vec<_>>();
            self.auth_type = args
                .get(1)
                .ok_or(anyhow!("500 The authentication type is incorrect.\r\n"))?
                .trim_end()
                .to_string();
            Some(args)
        } else {
            None
        };

        match self.auth_type.to_uppercase().as_str() {
            "PLAIN" => {
                let auth_plain = if args.is_some() && args.as_ref().unwrap().len() == 3 {
                    args.unwrap()[2].trim_end()
                } else {
                    if self.status.lock == LockMode::AUTH {
                        self.status.lock = LockMode::NULL;
                        request.trim_end()
                    } else {
                        self.status.lock = LockMode::AUTH;
                        return Ok("334 \r\n".to_string());
                    }
                };
                if auth_plain == plain_encode(&self.user, &self.passwd) {
                    self.status.auth = true;
                    return Ok("235 Authentication successful\r\n".to_string());
                }
            }
            "LOGIN" => {
                if self.status.lock == LockMode::NULL && !self.status.auth_login_begin {
                    self.status.lock = LockMode::AUTH;
                    self.status.auth_login_begin = true;
                    return Ok("334 VXNlcm5hbWU6\r\n".to_string());
                } else if self.status.auth_login_begin
                    && request.trim_end() == BASE64_STANDARD.encode(&self.user)
                {
                    self.status.auth_login_begin = false;
                    return Ok("334 UGFzc3dvcmQ6\r\n".to_string());
                } else if request.trim_end() == BASE64_STANDARD.encode(&self.passwd) {
                    self.status.auth = true;
                    self.status.lock = LockMode::NULL;
                    return Ok("235 Authentication successful\r\n".to_string());
                }
            }
            _ => {
                self.status.quit = true;
                return Err(anyhow!("500 The authentication type is incorrect.\r\n"));
            }
        };

        self.status.quit = true;
        return Err(anyhow!("535 Authentication failed\r\n".to_string()));
    }

    async fn starttls(&mut self) -> Result<String, anyhow::Error> {
        if !self.tls_type.is_some() {
            return Err(anyhow!(
                "454 TLS not available due to temporary reason\r\n".to_string()
            ));
        }
        self.status.has_tls = true;
        self.status.starttls = true;
        Ok("220 Ready to start TLS\r\n".to_string())
    }
}
