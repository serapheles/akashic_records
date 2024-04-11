use std::{thread, thread::sleep, time};
use std::collections::HashSet;
use std::error::Error;

use log::*;
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
};

// use crate::api_handler::*;
use crate::file_parser::*;
use crate::stream::StreamManager;

mod api_handler;
mod stream;
mod file_parser;

//Loop to periodically call the HoloDex API to find new streams.
//Checks against a hashset to determine if a stream is already downloaded, has been checked before,
//or has changes to the title (that may change the status).
//TODO: There are too many string allocations, should restructure to use str references.
//TODO: found_list should be split into downloading and noticed hashsets.
//TODO: Consider making into a struct, to share the various data-structures, thus allowing easier
//  function changing/breakdowns.
//TODO: Passing via pipe.
//TODO: Chat logging.
fn primary_loop() -> Result<(), Box<dyn Error>> {
    let dex_key: String = read_key("resources/holoDexApiKey.txt");

    let client = reqwest::blocking::Client::new();
    let url = "https://holodex.net/api/v2/live";
    let params = [("type", "stream,placeholder"), ("max_upcoming_hours", "168")];
    let archive_set: HashSet<String> = read_set("resources/archiveList.txt");
    let check_set: HashSet<String> = read_set("resources/checkList.txt");
    let key_set: Vec<String> = read_vec("resources/keyList.txt");
    let mut found_set: HashSet<String> = HashSet::new();
    let mut tier_2 = archive_set.clone();

    tier_2.extend(read_set("resources/corps/allAtelier.txt"));
    tier_2.extend(read_set("resources/corps/allEien.txt"));
    tier_2.extend(read_set("resources/corps/allHolo.txt"));
    tier_2.extend(read_set("resources/corps/allIdolCorp.txt"));
    tier_2.extend(read_set("resources/corps/allNiji.txt"));
    tier_2.extend(read_set("resources/corps/allPhase.txt"));
    tier_2.extend(read_set("resources/corps/allPrism.txt"));
    tier_2.extend(read_set("resources/corps/allVoms.txt"));
    tier_2.extend(read_set("resources/corps/allVshojo.txt"));

    let url = reqwest::Url::parse_with_params(url, params)?;
    let request = client.get(url).header("X-APIKEY", dex_key);

    loop {
        let response = loop {
            let res = request.try_clone().unwrap().send();

            match res {
                Ok(val) => {
                    if val.status().is_success(){
                        //If this doesn't work, everything is probably borked and won't work,
                        //so no point in correcting.
                        break val.json::<serde_json::Value>().unwrap_or_else(|error|{
                            panic!("Error parsing response to json: {:?}.", error);
                        })
                    } else {
                        error!("Bad response status: {:?}.", val.status());
                    }
                }
                Err(error) => {error!("Request failed: {:?}.", error)}
            };

            sleep(time::Duration::from_secs(120));
        };

        //I would love to pass more references instead of cloning and creating new objects, but this
        //gets around issues with moved and borrowed values.
        for val in response.as_array().expect("Error in converting from Json to Array"){
            //Confirmed youtube stream
            if val["type"] == "stream"{
                if !tier_2.contains(val["channel"]["id"].as_str().unwrap()) ||
                    found_set.contains(val["id"].as_str()
                        .unwrap()){continue}

                if archive_set.contains(val["channel"]["id"].as_str()
                    .unwrap()) {
                    let id = val["id"].as_str().unwrap();
                    found_set.insert(id.to_string());
                    info!("Stream found from api: {}", id);
                    stream_capture(id.to_string());
                    continue
                }

                let mut title = val["title"].as_str()
                    .unwrap().to_lowercase();
                title.retain(|c| !c.is_whitespace());

                if check_set.contains(val["channel"]["id"].as_str()
                    .unwrap()){
                    for key in &key_set{
                        if title.contains(key.as_str()){
                            let id = val["id"].as_str().unwrap();
                            found_set.insert(id.to_string());
                            info!("Stream found from api: {}", id);
                            stream_capture(id.to_string());
                            continue
                        }
                    }

                    //Consider adding/checking topic_id?
                }

                if title.contains("unarchive"){
                    let id = val["id"].as_str().unwrap();
                    found_set.insert(id.to_string());
                    info!("Stream found from api: {}", id);
                    stream_capture(id.to_string());
                    continue
                }

                //We could move this up, create an external bool (it could be twitter), drop the stream check
                //and then pass the capture value based on the external bool. ID should be safe for the set,
                //and the channel id should be safe/consistent.

            } else if val["placeholderType"] == "external-stream" && val["status"] == "live"{
                //Twitch stream.
                println!("{}", val);
            }
        }

        sleep(time::Duration::from_secs(120));
    }
}

//Function to start a download in a separate thread. Realistically, this is one line of code, but
//having it split this way makes for easier testing and future changes.
//Ideally, IDs would be removed from the found list after completion to free resources, but realistically
//a year's worth of stream ids would still be relatively inconsequential.
//TODO: Should log when a thread closes. May want to check for thread issues (such as "freezes").
fn stream_capture(target: String) -> thread::JoinHandle<()> {
    thread::spawn(|| {
        StreamManager::new(target).expect("Error setting up StreamManager.").download_loop();
    })
}

//Main function. Sets up the logging file and calls either the primary loop function, or (a) testing
// function(s). Had issues setting up pyo3_pylogger, so doing without for now.
fn main() -> Result<(), Box<dyn Error>> {
    //Basic logging. Current needs are simple, so not a lot of effort was spent on this.
    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d(%Y-%m-%d %H:%M:%S)} - {l}: {m}{n}",
        )))
        .build("resources/akashic.log")?;

    let config = Config::builder().appender(Appender::builder()
        .build("logfile", Box::new(logfile)))
        .build(Root::builder().appender("logfile")
            .build(LevelFilter::Info))?;

    //Adds panics to logging. Best as I can tell, this adds a hook to the log crate, from which the
    //log4rs builds on, so they seem to play well with each other.
    log_panics::init();

    //Do I actually need a reference?
    let _handle = log4rs::init_config(config)?;

    primary_loop()
}