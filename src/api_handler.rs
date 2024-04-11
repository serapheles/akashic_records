use reqwest::blocking::Response;
use serde_json::Value;
use crate::file_parser;

//Non-async basic API call, mostly for testing.
pub fn _api_basic(target: &str) -> Value {
    let response = reqwest::blocking::get(target);
    let json = match response {
        Ok(response) => response.json::<serde_json::Value>(),
        //This isn't acceptable long term behavior.
        Err(error) => panic!("Request failed: {:?}", error),
    };
    //This is assuming that a request that was returned succeeded.
    json.unwrap()
    // println!("{}", api_basic("https://httpbin.org/ip"))
}

pub fn _api_header(target: &str, header: &str) {
    let client = reqwest::blocking::Client::new();
    let res = client.get(target).header(header, "").send().unwrap();
}

//As a struct to hold the reqwest for reuse without re-creating each time.
//A blocking request makes sense for how it's being used, but it's feasible that things could be
//structured such that a non-blocking request would be useful.
pub struct DexClient {
    request: reqwest::blocking::RequestBuilder,
}

//Not as organic as I would like in Rust, but this allows a clean call without needing to create a
//new client or rebuild the RequestBuilder (albeit, by cloning the RequestBuilder).
impl DexClient {
    pub fn new(header: &str) -> DexClient {
        let client = reqwest::blocking::Client::new();
        let url = "https://holodex.net/api/v2/live";
        let params = [("type", "stream,placeholder"), ("max_upcoming_hours", "168")];

        let url = reqwest::Url::parse_with_params(url, params).unwrap();
        let request = client.get(url).header("X-APIKEY", header);
        DexClient { request }
    }

    //Checks the currently live and upcoming streams from the HoloDex API.
    //Only checks the passed channels, as declared in new().
    pub fn dex_live_scan(&self) -> reqwest::Result<Response> {
        self.request.try_clone().unwrap().send()
    }
}

//Parameters and return value types depend on the requests alternative
pub fn _dex_api(header: String, target: String) -> reqwest::Result<Response> {
    let client = reqwest::blocking::Client::new();
    let url = "https://holodex.net/api/v2/videos/".to_string() + &target;
    client.get(url).header("X-APIKEY", header).send()
    // let res = client.get(url).header("X-APIKEY", header).send().unwrap();
    // println!("{}", res.json::<serde_json::Value>().unwrap())
}

//Returns information on a single video from the HoloDex API.
pub fn _dex_video(header: String, video: String) -> reqwest::Result<Response> {
    let client = reqwest::blocking::Client::new();
    let url = "https://holodex.net/api/v2/videos/".to_string() + &video;
    client.get(url).header("X-APIKEY", header).send()
    // let response = client.get(url).header("X-APIKEY", header).send();
    // let json = match response {
    //     Ok(response) => response.json::<serde_json::Value>(),
    //     //This isn't acceptable long term behavior.
    //     Err(error) => panic!("Request failed: {:?}", error),
    // };
    // //This is assuming that a request that was returned succeeded.
    // json.unwrap()
}

pub fn _chan_info(header: &str, channel: &str) -> reqwest::Result<Response> {
    let client = reqwest::blocking::Client::new();
    let url = "https://holodex.net/api/v2/channels/".to_string() + channel;
    client.get(url).header("X-APIKEY", header).send()
}

//Returns all upcoming and live videos, not just those from a list of channels. May end up swapping
//to this for expanded title sifting, possibly as a less frequent call.
pub fn _dex_all_live(header: String) -> reqwest::Result<Response> {
    let client = reqwest::blocking::Client::new();
    let url = "https://holodex.net/api/v2/live";
    client.get(url).header("X-APIKEY", header).send()
    // let response = client.get(url).header("X-APIKEY", header).send();
    // let json = match response {
    //     Ok(response) => response.json::<serde_json::Value>(),
    //     //This isn't acceptable long term behavior.
    //     Err(error) => panic!("Request failed: {:?}", error),
    // };
    // //This is assuming that a request that was returned succeeded.
    // json.unwrap()
    // // println!("{}", res.json::<serde_json::Value>().unwrap())
}

pub fn _holo_chans(header: &str){
    dex_channels(header, "Hololive", None);
}

pub fn _niji_chans(header: &str){
    dex_channels(header, "Nijisanji", Some("en"))
}

pub fn _idol_chans(header: &str){
    dex_channels(header, "idol Corp", None);
}

pub fn _atelier_chans(header: &str){
    dex_channels(header, "Atelier Live", None);
}

pub fn _eien_chans(header: &str){
    dex_channels(header, "EIEN Project", None);
}

pub fn _vshojo_chans(header: &str){
    dex_channels(header, "VShojo", None);
}

pub fn _voms_chans(header: &str){
    dex_channels(header, "VOMS", None);
}

pub fn _prism_chans(header: &str){
    dex_channels(header, "PRISM", None);
}

pub fn _phase_chans(header: &str){
    dex_channels(header, "Phase Connect", None);
}

//Since the language option doesn't seem to work, the various org methods could be removed and
//directly passed to calls to this method, but frankly I don't expect to be able to remember each
//organization name and specific formatting.
//Should probably return the value(s) to be processed, but realistically it's doing what it's here for.
pub fn dex_channels(header: &str, organization: &str, languages: Option<&str>) {
    let client = reqwest::blocking::Client::new();
    let url = "https://holodex.net/api/v2/channels";

    let mut offset = 0;

    loop{
        let n = offset.to_string();
        //Unfortunately, the lang field appears to be unset for most channels.
        let url = match languages {
            None => reqwest::Url::parse_with_params(url, [("org", organization), ("limit", "50"), ("offset", n.as_str())]).expect(""),
            Some(x) => reqwest::Url::parse_with_params(url, [("org", organization), ("lang", x), ("limit", "50"), ("offset", n.as_str())]).expect("")
        };

        let res = client.get(url).header("X-APIKEY", header).send();
        match res {
            Ok(val) => {
                if val.status().is_success(){
                    //If this doesn't work, everything is probably borked and won't work,
                    //so no point in correcting.
                    let x = val.json::<serde_json::Value>().expect("");
                    //Should probably output to file rather than printing.
                    for index in x.as_array().expect("") {
                        println!("#{}", index["english_name"]);
                        println!("{}", index["id"]);
                    }
                } else {
                    break
                }
            }
            Err(error) => {break}
        };
        offset = offset + 50;
    }

}

//Calling the official youtube API allows checking information not found in the HoloDex API, or
//information that is more current. (The official API isn't always up-to-date itself).
//TODO: Add request for the official youtube API.
pub fn _google_api(_header: String, _target: String) {}
