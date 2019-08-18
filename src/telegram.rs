use serde::{Deserialize, Serialize};
use url::Url;

const API_URL: &str = "https://api.telegram.org";

#[derive(Debug, Deserialize)]
struct Response {
    ok: bool,
    description: Option<String>,
}

trait Type: Serialize {
    fn method() -> &'static str;
}

pub struct Client {
    url: Url,
    http: reqwest::Client,
}

impl Client {
    pub fn new(bot_token: &str) -> Client {
        let mut url = Url::parse(API_URL).unwrap();
        url.path_segments_mut()
            .unwrap()
            .push(&format!("bot{}", bot_token))
            .push("");
        Client {
            url,
            http: reqwest::Client::new(),
        }
    }

    fn post<T: Type>(&self, message: &T) -> reqwest::Result<()> {
        let _response = self
            .http
            .post(self.url.join(T::method()).unwrap())
            .json(&message)
            .send()?
            .json::<Response>();
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct Message {
    chat_id: i64,
    text: String,
    parse_mode: String,
    disable_web_page_preview: bool,
}

impl Type for Message {
    fn method() -> &'static str {
        "sendMessage"
    }
}

impl Message {
    pub fn new(chat_id: i64, text: String) -> Message {
        Message {
            chat_id,
            text,
            parse_mode: "Markdown".to_string(),
            disable_web_page_preview: true,
        }
    }

    pub fn send(self, client: &Client) -> crate::Result<()> {
        Ok(client.post(&self)?)
    }
}
