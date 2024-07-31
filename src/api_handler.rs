use std::error::Error;
use std::fs;

use google_youtube3::{oauth2, YouTube};
use google_youtube3::api::Video;
use google_youtube3::oauth2::{AccessToken, ServiceAccountAuthenticator};
use google_youtube3::oauth2::authenticator::Authenticator;
use hyper::client::HttpConnector;
use hyper_rustls::HttpsConnector;
use reqwest::{blocking, Client, Url};
use reqwest::blocking::Response;
use tracing::error;

pub struct DexClient {
    client: blocking::Client,
    header: String,
    url: Url,
}

impl DexClient {
    pub fn new(header: String) -> Self {
        let client = blocking::Client::new();
        let endpoint = "https://holodex.net/api/v2/live";
        let params = [("type", "stream,placeholder"), ("max_upcoming_hours", "168")];
        let url = Url::parse_with_params(endpoint, params).unwrap();

        DexClient {
            client,
            header,
            url,
        }
    }

    pub fn live_check(&self) -> reqwest::Result<Response> {        
        self.client.get(self.url.clone()).header("X-APIKEY", &self.header).send()
    }
}

//Parameters and return value types depend on the requests alternative
pub fn _dex_api(header: String, target: String) -> reqwest::Result<Response> {
    let client = blocking::Client::new();
    let url = "https://holodex.net/api/v2/videos/".to_string() + &target;
    client.get(url).header("X-APIKEY", header).send()
    // let res = client.get(url).header("X-APIKEY", header).send().unwrap();
    // println!("{}", res.json::<serde_json::Value>().unwrap())
}

pub fn _chan_info(header: &str, channel: &str) -> reqwest::Result<Response> {
    let client = blocking::Client::new();
    let url = "https://holodex.net/api/v2/channels/".to_string() + channel;
    client.get(url).header("X-APIKEY", header).send()
}

pub fn _holo_chans(header: &str) {
    _dex_channels(header, "Hololive", None);
}

pub fn _niji_chans(header: &str) {
    _dex_channels(header, "Nijisanji", Some("en"))
}

pub fn _idol_chans(header: &str) {
    _dex_channels(header, "idol Corp", None);
}

pub fn _atelier_chans(header: &str) {
    _dex_channels(header, "Atelier Live", None);
}

pub fn _eien_chans(header: &str) {
    _dex_channels(header, "EIEN Project", None);
}

pub fn _vshojo_chans(header: &str) {
    _dex_channels(header, "VShojo", None);
}

pub fn _voms_chans(header: &str) {
    _dex_channels(header, "VOMS", None);
}

pub fn _prism_chans(header: &str) {
    _dex_channels(header, "PRISM", None);
}

pub fn _phase_chans(header: &str) {
    _dex_channels(header, "Phase Connect", None);
}

//Since the language option doesn't seem to work, the various org methods could be removed and
//directly passed to calls to this method, but frankly I don't expect to be able to remember each
//organization name and specific formatting.
//Should probably return the value(s) to be processed, but realistically it's doing what it's here for.
pub fn _dex_channels(header: &str, organization: &str, languages: Option<&str>) {
    let client = blocking::Client::new();
    let url = "https://holodex.net/api/v2/channels";

    let mut offset = 0;

    loop {
        let n = offset.to_string();
        //Unfortunately, the lang field appears to be unset for most channels.
        let url = match languages {
            None => Url::parse_with_params(url, [("org", organization), ("limit", "50"), ("offset", n.as_str())]).unwrap(),
            Some(x) => Url::parse_with_params(url, [("org", organization), ("lang", x), ("limit", "50"), ("offset", n.as_str())]).unwrap()
        };

        let res = client.get(url).header("X-APIKEY", header).send();
        match res {
            Ok(val) => {
                if val.status().is_success() {
                    let x = val.json::<serde_json::Value>().unwrap();
                    // TODO: Should output to file rather than printing.
                    for index in x.as_array().unwrap() {
                        println!("#{}", index["english_name"]);
                        println!("{}", index["id"]);
                    }
                } else {
                    break;
                }
            }
            Err(_error) => { break }
        };
        offset += 50;
    }
}

async fn get_auth() -> Authenticator<HttpsConnector<HttpConnector>> {
    let key =
        oauth2::read_service_account_key("res/keys/client_secret.json").await.unwrap();
    ServiceAccountAuthenticator::builder(key).build().await.unwrap()
}

//Calling the official YouTube API allows checking information not found in the HoloDex API, or
//information that is more current. (Unsure if the official API is always up-to-date itself).
// Also, this crate this function uses involves very out of date
pub async fn google_api(target: String) -> Result<Video, Box<dyn Error>> {
    if target.len() != 11 {
        return Err(Box::<dyn Error>::from("Invalid API target, must be an 11 character video id."));
    }

    let key = match fs::read_to_string("res/keys/google_key.txt") {
        Ok(file) => {
            file.trim().to_string()
        }
        Err(err) => {
            error!("Error reading Google key: {:?}", err);
            return Err(err.into());
        }
    };

    //Taken from the example from the google_youtube3 docs; untouched so I don't mess anything up.
    //Would have preferred to use a reqwest
    let client = hyper::Client::builder()
        .build(hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots().unwrap()
            .https_or_http().enable_http1().build());

    let youtube = YouTube::new(client, get_auth().await);

    let request = youtube.videos()
        .list(&vec![String::from("snippet")])
        .add_id(target.as_str())
        .param("key", &*key);

    match request.doit().await {
        Ok(res) => {
            Ok(res.1.items.unwrap().pop().unwrap())
        }
        Err(err) => {
            Err(err.into())
        }
    }
}