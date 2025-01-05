# SMTP to Lark mail API
由于直接使用 Lark 提供的 SMTP 服务会泄漏发送者的 IP 地址，而通过 Lark 提供的发送用户邮件 API 不会包含发送者 IP 地址， 所以诞生了这个项目，原理为提供一个 SMTP 服务器，客户端使用 SMTP 协议发送邮件到此服务端，服务端将邮件发送到 Lark 提供的 API 。

## 使用方法
### 构建
1. 安装 Rust [https://www.rust-lang.org/zh-CN/tools/install](https://www.rust-lang.org/zh-CN/tools/install)
2. 安装依赖包
```bash
# Ubuntu / Debian
sudo apt install pkg-config

# Fedora / CentOS / RHEL
sudo dnf install pkg-config

# Windows
scoop install pkg-config
```
3. 拉取源代码并构建
```bash
git clone https://github.com/lingxh/smtp2larkapi
cd smtp2larkapi
cargo build --release
```
构建完成后的程序位于 `target/smtp2larkapi`


### 创建 Lark App
1. 前往 [https://open.larksuite.com/](https://open.larksuite.com/) 创建一个企业自建应用，应用名称和信息随便填写
2. 开发配置 - 权限管理 - API 权限,  搜索 `mail:user_mailbox.message:send` 并勾选
3. 开发配置 - 安全设置 - 重定向 URL,  添加 `http://127.0.0.1:11451`
4. 应用发布 - 版本管理与发布 - 创建版本, 应用版本号和更新说明随便填写，提交后在弹出的"确认提交发布申请"窗口点击"申请线上发布"，若提示需要审核请使用管理员账号前往 管理后台 - 工作台 - 应用审核 处通过审核
5. 基础信息 - 凭证与基础信息,  保存 `App ID` 和 `App Secret`

### 软件配置
1. 在本软件所在目录下创建一个 `data` 文件夹，并进入该文件夹
2. 创建 `config.json` 文件，并按照以下模板写入内容:
```json
{
    "listener" : "0.0.0.0:587",
    "host" : "smtp.gmail.com",
    "user": "user",
    "default_name": "test",
    "passwd": "password",
    "safety": "starttls",
    "tls": {
        "cert": "data/code.crt",
        "key": "data/code.key"
    }
}
```
配置项解释：  
 `listener`     : 监听地址  
 `host`         : SMTP 服务器主机名  
 `user`         : SMTP 鉴权用户名  
 `default_name` : 选填，默认发件人名称  
 `passwd`       : SMTP 鉴权密码  
 `safety`       : 加密类型，可选择 no, ssl, starttls 三者之一  
 `tls`          : 选填，若 safety 配置为 no 则不需要填写  
 `cert`         : tls证书  
 `key`          : tls密钥  


3. 创建 `app_info.json` 文件，并按照以下模板写入内容:
```json
{
    "app_id": "",
    "app_secret": "",
    "code": ""
}
```
配置项解释：  
 `app_id`    : App ID    
 `app_secret`: App Secret  
 `code`      : 登录授权码  
 
注意：登录授权码有效期只有5分钟，请获取填入后立即启动一次程序获得长效 Token， 后续若不出现连续30天未运行此程序则不再需要此项  

登录授权码获取方式：

访问 https://open.larksuite.com/open-apis/authen/v1/authorize?app_id={app_id}&redirect_uri=http://127.0.0.1:11451&scope=mail:user_mailbox.message:send   

注意将 {app_id} 替换为你自己的 AppID   
然后单击授权，会跳转到 http://127.0.0.1:11451 ,复制 URL 上的 code 参数即是登录授权码  

4. 运行程序，程序会自动获取 Token，若出现连续30天未运行此程序则 Token 失效，需要重新获取授权码并更新 `app_info.json` 文件。

## 最后
如果此项目帮助到了你，请点一个 Star ，不胜感激  
如果你此项目运行有任何问题或有改进建议，欢迎发布 issues

# SMTP to Lark Mail API

Since directly using Lark's SMTP service exposes the sender's IP address, while sending emails through Lark's Mail API does not include the sender's IP, this project was created. The principle is to provide an SMTP server that accepts emails sent via SMTP protocol from the client, and then the server sends those emails to Lark's API.

## How to Use
### Build
1. Install Rust [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install)
2. Install dependencies
```bash
# Ubuntu / Debian
sudo apt install pkg-config git

# Fedora / CentOS / RHEL
sudo dnf install pkg-config git

# Windows
scoop install pkg-config git
```
3. Build
```bash
git clone https://github.com/lingxh/smtp2larkapi
cd smtp2larkapi
cargo build --release
```
The built program will be located at `target/smtp2larkapi`. 

### Create a Lark App
1. Go to [https://open.larksuite.com/](https://open.larksuite.com/) and create an enterprise self-built application. You can fill in any application name and details.
2. Development Configuration - Permissions & Scopes - API Scopes, search for `mail:user_mailbox.message:send`, and check it.
3. Development Configuration - Security Settings - Redirect URLs, and add `http://127.0.0.1:11451`.
4. App Versions - Version Management & Release, create a version, and fill in any version number and release notes. After submission, click "Submit the release request" in the pop-up window. If prompted for a review, use an administrator account to go to Admin Console - Workbench - App Review and approve the review.
5. Credentials & Basic Info - Credentials, and save the `App ID` and `App Secret`.

### Software Configuration
1. Create a `data` folder in the directory where the software is located, and enter this folder.
2. Create a `config.json` file, and write the content according to the following template:
```json
{
    "listener": "0.0.0.0:587",
    "host": "smtp.gmail.com",
    "user": "user",
    "default_name": "test",
    "passwd": "password",
    "safety": "starttls",
    "tls": {
        "cert": "data/code.crt",
        "key": "data/code.key"
    }
}
```
Explanation of configuration items:

`listener`: Listening address  
`host`: SMTP server hostname  
`user`: SMTP authentication username  
`passwd`: SMTP authentication password  
`default_name`: Optional, Sender's name  
`safety`: Encryption type, options are no, ssl, or starttls  
`tls`: Optional, not required if safety is set to no  
`cert`: TLS certificate  
`key`: TLS private key  

3. Create an `app_info.json` file and write the content according to the following template:
```json
{
    "app_id": "",
    "app_secret": "",
    "code": ""
}
```
Explanation of configuration items:

`app_id`: App ID  
`app_secret`: App Secret  
`code`: Login authorization code  

Note: The login authorization code is only valid for 5 minutes. After obtaining and entering it, immediately start the program once to acquire a long-term token. If the program is run regularly, you won't need this again unless you skip running it for 30 consecutive days.

How to obtain the login authorization code:

Visit https://open.larksuite.com/open-apis/authen/v1/authorize?app_id={app_id}&redirect_uri=http://127.0.0.1:11451&scope=mail:user_mailbox.message:send

Replace `{app_id}` with your App ID. After authorization, it will redirect to http://127.0.0.1:11451. Copy the code parameter from the URL, which is your login authorization code.



4. Run the program. It will automatically acquire the Token. If the program is not run for 30 consecutive days, the token will expire, and you'll need to obtain a new authorization code and update the `app_info.json` file.

## Finally
If this project helped you, please give it a star; I would greatly appreciate it!   
If you encounter any issues while running this project or have any suggestions for improvement, feel free to open an issue.
