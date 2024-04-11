use std::{thread, time};
use std::collections::HashMap;
use std::error::Error;

use log::*;
use pyo3::prelude::*;
use pyo3::types::{IntoPyDict, PyDict};

//Using a struct as a vaguely OOP approach doesn't feel quite as fluent in Rust as it does in Python
//or Java, but it 1) closer fits the structure/logic I've used in the past, and 2) is good
//experience working with structs in Rust. In the future, may make some of the functions traits and
//split download attempts to be platform specific.
//TODO: Add explicit support for other platforms (twitch, twitter, et cetera).


//Struct to compartmentalize the downloading of a given stream.
//Some of these would be useful as consts, but I see no clean way to do that.
#[pyclass]
pub struct StreamManager {
    yt_dlp: Py<PyModule>,
    opts: Py<PyDict>,
    kwargs: Py<PyDict>,
    target: String,
    cookie_check: bool,
    class: PyObject,
    wait_time: u64,
    completed: bool
}

#[pymethods]
impl StreamManager{

    fn hook(&mut self, dict: Py<PyAny>){
        self.completed = true;
    }

    //we could write a function that explicitly creates and returns an inner function that takes the
    //dict argument. would it work with the class restriction(s)?
}

impl StreamManager {
    //Functionally a constructor, sets up the options et cetera to pass to the downloader.
    pub fn new(url: String) -> Result<StreamManager, Box<dyn Error>> {
        Python::with_gil(|py| {
            //Most of these should always succeed, but this first one may fail if yt_dlp isn't
            //available. "Best practices" encourage a venv, but that's not necessary anywhere I plan
            //on running this.
            let yt_dlp: Py<PyModule> = py
                .import("yt_dlp")
                .expect("Error importing yt-dlp, check that it is available.")
                .into();
            let mut opts = HashMap::new();
            // opts.insert("quiet".to_string(), true);
            opts.insert("writeinfojson".to_string(), true);
            opts.insert("nooverwrite".to_string(), true);
            opts.insert("nopart".to_string(), true);
            opts.insert("hls_use_mpegts".to_string(), true);
            //For Testing. DOES NOT WORK WITH THE COMPLETED FLAG!
            // opts.insert("skip_download".to_string(), true);


            let opts: &PyDict = opts.into_py_dict(py);
            let kwargs: &PyDict = PyDict::new(py);
            let paths: &PyDict = PyDict::new(py);

            //Sets download folders. I would prefer for the temp folder to be separate, rather than
            //nested, but that's a problem for another day.
            paths.set_item("temp", "active")?;
            paths.set_item("home", "downloads")?;

            //There are other options that may be useful, but these are the key options.
            opts.set_item("paths", paths).unwrap();
            opts.set_item("writethumbnail", true)?;
            opts.set_item("socket_timeout", 60)?; //Idk if there is an ideal value here.

            //Icky. I assume that the provided method is called with the call_method, so I suppose it
            //returns an updated method to check on for the actual filter.
            //Ostensibly, this should work for a post_hook as well.
            opts.set_item("match_filter",
                          py.import("yt_dlp.utils")?
                              .getattr("_utils")?
                              .call_method1("match_filter_func", ("is_live", ))?)
                .expect("Failed to add filter , may be issue with yt-dlp path.");

            kwargs.set_item("params", opts).unwrap();

            let target = url.to_string();
            let kwargs = kwargs.into();
            let opts = opts.into();

            //Error type. There may be a better way to do this, but documentation is sparse on how
            //I want to be using py03.
            let class: PyObject = py
                .import("yt_dlp.utils")?
                .getattr("DownloadError")?
                .to_object(py);

            // let mut sm = StreamManager {
            //     yt_dlp,
            //     opts,
            //     kwargs,
            //     target,
            //     cookie_check: false,
            //     class,
            //     wait_time: 1,
            //     completed: false,
            // };
            //
            // opts.set_item("progress_hooks", wrap_pyfunction!(sm.hook))?;
            //
            // Ok(sm)

            Ok(StreamManager {
                yt_dlp,
                opts,
                kwargs,
                target,
                cookie_check: false,
                class,
                wait_time: 1,
                completed: false,
            })
        })
    }

    //Maintains the primary attempt loop. Realistically, most attempts are for streams that are yet
    //to start, and thus fail.
    pub fn download_loop(&mut self) {
        loop {
            let result = self.download_attempt();
            match result {
                Ok(_) => {
                    info!("{}: Download attempt complete.", self.target);
                    //This is to catch streams over 6 hours long. Should also catch unexpected drops
                    //(such as the streamer losing internet).
                    //Keep an eye out for related issues.
                    self.wait_time = 0;
                }
                Err(e) => {
                    self.error_parse(e);
                    if self.completed {
                        break
                    }
                    thread::sleep(time::Duration::from_secs(self.wait_time));
                }
            }
        }
    }

    //Attempts to download the stream. In the case of a DownloadError, returns the error as a string
    //Other errors lead to a panic.
    //TODO: Add a progress-hook to log when a stream has "completed" but before it returns (for tracking down a specific situation/potential error)
    fn download_attempt(&self) -> Result<(), String> {
        Python::with_gil(|py| {
            let ydl = self
                .yt_dlp
                .as_ref(py)
                .getattr("YoutubeDL")
                .unwrap()
                .call((), Some(self.kwargs.as_ref(py)))
                .unwrap();

            let args = (self.target.as_str(),);
            match ydl.call_method1("download", args) {
                Ok(x) => {
                    println!("{:?}", x);
                    Ok(())
                },
                Err(error) => {
                    //There should be a better way to do this
                    //These should also be broken down better, but that's on the library
                    if self.class.is(error.get_type(py)) {
                        Err(error.to_string())
                    } else {
                        // error!("Unchecked error on {}: {:?}", self.target, error);
                        //Panics should be logged now
                        panic!("Unchecked error on {}: {:?}", self.target, error);
                    }
                }
            }
        })
    }

    //TODO: For hard time shifts, should check again at the min of 5 after the hour or current wait.
    fn error_parse(&mut self, error: String) {
        let mut msg = error.rsplit(" ");
        //Ordered by expected frequency.
        match msg.next().unwrap() {
            "moments." | "shortly" => {
                self.wait_time = 15;
                //x:05
            }
            "minutes." | "minutes" => {
                self.wait_time = msg.next().unwrap().parse().unwrap();
                info!("{}: Scheduled for {} minutes.", self.target, self.wait_time);
                self.wait_time *= 48;
                //x:05
            }
            "hours." | "hours" => {
                self.wait_time = msg.next().unwrap().parse().unwrap();
                info!("{}: Scheduled for {} hours.", self.target, self.wait_time);
                self.wait_time *= 2880;
            }
            "days." | "days" => {
                self.wait_time = msg.next().unwrap().parse().unwrap();
                info!("{}: Scheduled for {} days.", self.target, self.wait_time);
                self.wait_time *= 69120;
            }
            //Generally just chat rooms.
            "years." | "years" => {
                self.wait_time = msg.next().unwrap().parse().unwrap();
                info!("{}: Scheduled for {} years.", self.target, self.wait_time);
                self.wait_time *= 69120;
            }
            //Checks for member streams and other results. For member streams, first checks a
            //flag. If the flag is set, an authentication attempt has failed so wait_time is set
            //to 0 in order to naturally end the attempt. Otherwise, adds the cookiefile to the
            //passed options. Other error messages are logged, with the match pattern allowing
            //easy updates.
            "perks." => {
                warn!("{}: Member stream", self.target);
                if !self.cookie_check {
                    self.cookie_check = true;
                    //There is a reason why this is done here, rather than with the rest of
                    //the options, but it's not relevant to the code.
                    Python::with_gil(|py| {
                        let opts = self.opts.as_ref(py);
                        opts.set_item("cookiefile", "resources/cookies.txt")
                            .unwrap();
                    });
                } else {
                    warn!("{}: Failed membership authentication.", self.target);
                    self.completed = true
                }
            }
            //Found when a stream is offline, after having started. May be used elsewhere.
            "difficulties." | "difficulties" => {
                self.wait_time = 10;
            }

            val => {
                error!("{}: Unsupported error message: {}", self.target, val);
                self.completed = true
            }
        }
    }
}