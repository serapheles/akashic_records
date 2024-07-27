use std::{fs, thread, thread::sleep, time};
use std::collections::{HashSet, VecDeque};
use std::error::Error;
use futures::executor::block_on;
use serde_json::Value;
use tracing::{debug, error, info, Level, span, subscriber, warn, trace};
use tracing::{info_span, Span};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_panic::panic_hook;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, Registry};
use tracing_subscriber::fmt::time::ChronoLocal;
use crate::api_handler::*;
use crate::stream::StreamManager;

mod api_handler;
mod stream;

// Base file parsing function. Not entirely happy with returning a VecDeque, but it works for now.
fn read_file(file_name: &str) -> Result<VecDeque<String>, Box<dyn Error>> {
    match fs::read_to_string(file_name) {
        Ok(file) => {
            Ok(file.lines()
                .filter(|c| !c.starts_with('#'))
                .map(|c| c.trim().to_string())
                .collect::<VecDeque<String>>())
        }
        Err(err) => {
            error!("Error reading file {}: {:?}", file_name, err);
            Err(err.into())
        }
    }
}

// Loop to periodically call the HoloDex API to find new streams.
// Checks against a hashset to determine if a stream is already downloaded, has been checked before,
// or has changes to the title (that may change the status).
// TODO: found_list should be split into downloading and noticed hashsets.
// TODO: Consider making into a struct, to share the various data-structures, thus allowing easier
//  function changing/breakdowns.
// TODO: Passing via pipe.
// TODO: Chat logging.
fn api_loop() -> Result<(), Box<dyn Error>> {
    let dex_key = match read_file("res/keys/holodex_Key.txt") {
        Ok(mut file) => {
            file.pop_front().unwrap()
        }
        Err(err) => {
            panic!("Error reading key file: {:?}", err);
        }
    };
    let archive_set: HashSet<String> = match read_file("res/lists/archive_list.txt") {
        Ok(file) => {
            HashSet::from_iter(file)
        }
        Err(err) => {
            panic!("Error reading archive list: {:?}", err)
        }
    };
    let check_set: HashSet<String> = match read_file("res/lists/check_list.txt") {
        Ok(file) => {
            HashSet::from_iter(file)
        }
        Err(err) => {
            panic!("Error reading check list: {:?}", err)
        }
    };
    // Unlike the above, this one being empty isn't a deal-breaker.
    let key_word_set = read_file("res/lists/key_words.txt").unwrap_or_else(|e| {
        error!("Error reading keyword list: {:?}", e);
        VecDeque::new()
    });
    let mut found_set: HashSet<String> = HashSet::new();
    let api_caller = DexClient::new(dex_key);
    loop {
        debug!("Start of loop, checking for API response.");
        let response: Value = loop {
            match block_on(api_caller.live_check()) {
                Ok(val) => {
                    if val.status().is_success() {
                        debug!("Response status is success");
                        break block_on(val.json::<Value>()).unwrap();
                    } else {
                        error!("Bad response status: {:?}.", val.status());
                    }
                }
                Err(error) => {
                    error!("Request failed: {:?}.", error)
                }
            };
            // On failed request, wait two minutes before trying again.
            // Generally, this is either from calling before the device has connected to the internet
            // or because HoloDex is down.
            debug!("Starting response sleep");
            sleep(time::Duration::from_secs(120));
        };

        debug!("Starting response loop.");
        for val in response.as_array().unwrap() {
            if found_set.contains(val["id"].as_str().unwrap()) {
                debug!("Re-found a stream");
                continue;
            }

            if archive_set.contains(val["channel"]["id"].as_str().unwrap()) {
                if let Some(id) = target_parse(val) {
                    found_set.insert(id);
                }
                continue;
            }

            let mut title = val["title"].to_string().to_lowercase();
            title.retain(|c| !c.is_whitespace());

            if check_set.contains(&val["channel"]["id"].to_string()) {
                for word in &key_word_set {
                    if title.contains(word) {
                        if let Some(id) = target_parse(val) {
                            found_set.insert(id);
                        }
                        continue;
                    }
                }
            }

            if title.contains("unarchived") {
                if let Some(id) = target_parse(val) {
                    found_set.insert(id);
                }
                continue;
            }
            // If for some reason a stream isn't caught, this will show if it was overlooked or
            // somehow failed to be seen at all.
            debug!("Stream found and ignored: {}", val["id"]);
        }
        debug!("Starting loop sleep.");
        sleep(time::Duration::from_secs(120));
    }

}

// Usually the stream to download is a YouTube stream with a unique id, but other sources (Twitch)
// work differently.
fn target_parse(info: &Value) -> Option<String> {
    if info["type"] == "stream" {
        let mut id = info["id"].to_string();
        id.retain(|c| !c.eq(&'"'));
        info!("Stream found from api: {}", id);
        start_stream_loop(id.clone());
        Some(id)
    } else if info["placeholderType"] == "external-stream" && info["status"] == "live" {
        // Generally Twitch, but may pick up Twitter Spaces or other odd ball sources.
        let id = info["link"].to_string();
        info!("External stream found from api: {}", id);
        start_stream_loop(id);
        Some(info["id"].to_string())
    } else {
        // Most likely, this is an upcoming Twitch stream, but included (and at warn level) to ensure
        // nothing is slipping through.
        warn!("Stream checked, but failed the target parse: {}", info["id"]);
        None
    }
}

// Function to start a download in a separate thread. Realistically, this is one line of code, but
// having it split this way makes for easier testing and future changes.
// TODO: Set up spans for stream threads (probably before the struct is created, in the thread closure).
fn start_stream_loop(target: String) {
    thread::spawn(move || {
        // StreamManager::new(target.clone()).unwrap().download_loop();
        info!("{}: Thread ended.", target);
    });
}

// 1 sec Miko stream for testing: CAbEy8xAKSE
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    // Sets up a rolling log file.
    // There's a *lot* of components to the tracing logger, and they all had their own documentation,
    // but almost no clear examples as to how they fit together.
    // Tracing may be over kill for logging, but it's better documented than log4rs, specifically for
    // rolling-appenders.
    // Really wish I could shove all this in a function, but that doesn't seem to work for scoping
    // reasons that are unexpectedly annoying to get around.
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("akashic")
        .filename_suffix("log")
        .max_log_files(14)
        .build("logs/")
        .expect("Log file should have been created. Check file paths.");

    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Set up the logging format layer.
    // By default, ANSI escape characters are included that are illegible in a text reader.
    let fmt_layer = fmt::layer()
        // The pretty option is actually pretty nice for reading, but not great for actually parsing
        // logs. Good to turn on for reproducing an error.
        // .pretty()
        .with_timer(ChronoLocal::new(String::from("%d %b %Y - %T")))
        .with_ansi(false)
        .with_writer(non_blocking);

    // Set up the filter layer. Attempts to use the RUST_LOG env level, if it exists.
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("debug"))
        .unwrap();

    let subscriber = Registry::default()
        .with(filter_layer)
        .with(fmt_layer);
    
    subscriber::set_global_default(subscriber).unwrap();
    std::panic::set_hook(Box::new(panic_hook));

    api_loop()
}