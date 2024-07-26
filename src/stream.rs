use std::{cmp, thread, time};
use std::error::Error;

use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyList, PyTuple};
use tokio::runtime::Runtime;
use tokio::task;
use tracing::{debug, error, info, Level, span, subscriber, warn};

use crate::api_handler;

pub struct StreamManager {
    yt_dlp: PyObject,
    opts: Py<PyDict>,
    target: String,
    complete: bool,
    yt_error: PyObject,
    hook_struct: Py<PyStruct>,
}

#[pyclass]
struct PyStruct {
    pub yt_bool: bool,
    pub is_upcoming: bool,
    pub is_live: bool,
    pub was_live: bool,
}

#[pymethods]
impl PyStruct {
    // This feels incredibly hacky, but yt-dlp intentionally hides the info_dict
    // There may be util methods to access things, but this adds a lot of options for future development.
    #[pyo3(signature = (* args, * * kwargs))]
    fn hook(&mut self,
            _py: Python<'_>,
            args: &Bound<'_, PyTuple>,
            kwargs: Option<&Bound<'_, PyDict>>, ) -> PyResult<()> {
        let dict = match args.get_item(0) {
            Ok(val) => {
                val.downcast_into::<PyDict>()?
            }
            Err(err) => {
                error!("Failed to get dict from progress hook: {}", err);
                return Err(err);
            }
        };

        // May need to check 'was_live' and/or 'live_status' to ensure it doesn't stop halfway 
        // through a download of a non-live video 
        if dict.get_item("status")?.unwrap().eq("finished")? {
            match dict.get_item("info_dict")?.unwrap()
                .get_item("live_status")?
                .extract::<String>()?
                .as_str() {
                "is_upcoming" => {
                    // If a YouTube video is upcoming, the functions aren't even called, so this
                    // result would be of interest.
                    warn!("Video marked as upcoming.");
                    self.is_upcoming = true;
                }
                "is_live" => {
                    self.is_upcoming = false;
                    self.is_live = true;
                }
                "was_live" | "post_live" => {
                    self.is_live = false;
                    self.was_live = true;
                }
                "not_live" => {
                    // TODO: figure out what to do here.
                    self.is_live = false;
                    self.is_upcoming = false;
                    self.was_live = false;
                }
                _ => {
                    warn!("'None' value found for live_status.");
                }
            };
        }
        Ok(())
    }

    // Somewhat redundant with the hook function, but this sets stuff up early and can be expanded.
    // Would be nice to ensure this is only called once, at the beginning.
    #[pyo3(signature = (* args, * * kwargs))]
    fn pre_filter(&mut self,
                  _py: Python<'_>,
                  args: &Bound<'_, PyTuple>,
                  kwargs: Option<&Bound<'_, PyDict>>, ) -> PyResult<()> {
        //kwargs seems to have a dict of {"incomplete" : bool}

        let dict = match args.get_item(0) {
            Ok(val) => {
                val.downcast_into::<PyDict>()?
            }
            Err(err) => {
                error!("Failed to get dict from progress hook: {}", err);
                return Err(err);
            }
        };

        if dict.get_item("webpage_url_domain")?.unwrap()
            .eq("youtube.com")? {
            self.yt_bool = true;
        }

        self.is_live = dict.get_item("is_live")?.unwrap().downcast_exact::<PyBool>()?.is_true();
        self.was_live = dict.get_item("was_live")?.unwrap().downcast_exact::<PyBool>()?.is_true();

        Ok(())
    }
}

// TODO: Check what Miri carves in the desk.
impl StreamManager {
    pub fn new(mut target: String) -> Result<StreamManager, Box<dyn Error>> {
        // Most targets will be in the proper 11 character video id format, but this cleans up
        // values for testing, isolated usage, and future piped sources.
        // Since "twitch.tv/" is 10 characters, it's very unlikely (but not impossible, with url
        // shorteners or unexpected websites) that an 11 character value is not a YouTube video id.
        if target.contains("youtu") && target.len() > 11 {
            target = target.split_off(target.len() - 11);
        }

        Python::with_gil(|py| {
            let py_list = PyList::empty_bound(py);
            let params = PyDict::new_bound(py);
            let opts = Self::get_dict(py);
            let hook_struct = Py::new(py, PyStruct {
                yt_bool: false,
                is_upcoming: false,
                is_live: false,
                was_live: false,
            })?;

            py_list.append(hook_struct.getattr(py, "hook")?.to_object(py))?;
            opts.bind(py).set_item("progress_hooks", py_list)?;
            opts.bind(py).set_item("match_filter", hook_struct.getattr(py, "pre_filter")?.to_object(py))?;
            params.set_item("params", opts.bind(py))?;

            Ok(StreamManager {
                yt_dlp: Self::get_yt(py)?.call_bound(py, (), Some(&params))?,
                opts,
                target,
                complete: false,
                yt_error: Self::get_err_base(py),
                hook_struct,
            })
        })
    }

    // Returns a PyDict set to default values.
    fn get_dict(py: Python) -> Py<PyDict> {
        let dict = PyDict::new_bound(py);
        dict.set_item("writeinfojson", true).unwrap();
        dict.set_item("nopart", true).unwrap();
        dict.set_item("nooverwrite", true).unwrap();
        dict.set_item("hls_use_mpegts", true).unwrap();
        dict.set_item("writethumbnail", true).unwrap();
        dict.set_item("socket_timeout", 60).unwrap();

        // Sets download folders. I would prefer for the temp folder to be separate, rather than
        // nested, but that's a problem for another day.
        let paths = PyDict::new_bound(py);
        paths.set_item("temp", "active").unwrap();
        paths.set_item("home", "downloads").unwrap();
        dict.set_item("paths", paths).unwrap();

        // Notable/interesting options not used here:
        // ignoreerrors
        // logger
        // logtostderr
        // wait_for_video
        Bound::unbind(dict)
    }


    fn get_err_base(py: Python) -> PyObject {
        match PyModule::import_bound(py, "yt_dlp.utils") {
            Ok(yt) => {
                Bound::unbind(yt.getattr("YoutubeDLError").unwrap())
            }
            Err(err) => {
                panic!("Error importing yt-dlp, check that it is available: {:?}", err)
            }
        }
    }

    fn get_yt(py: Python) -> Result<PyObject, Box<dyn Error>> {
        match PyModule::import_bound(py, "yt_dlp") {
            Ok(yt) => {
                Ok(Bound::unbind(yt.getattr("YoutubeDL")?))
            }
            Err(err) => {
                panic!("Error importing yt-dlp, check that it is available: {:?}", err)
            }
        }
    }

    // Adds a filter to only download live streams.
    // Appears fairly gentle, but actually kills the thread or something?
    // Leaves the while loop but has an exit code 0?
    pub fn set_live_only(&mut self) {
        Python::with_gil(|py| {
            self.opts.bind(py).set_item("match_filter", PyModule::import_bound(py, "yt_dlp.utils").unwrap()
                .getattr("_utils").unwrap()
                .call_method1("match_filter_func", ("is_live",)).unwrap()).unwrap();

            let params = PyDict::new_bound(py);
            params.set_item("params", self.opts.bind(py)).unwrap();

            self.yt_dlp = Self::get_yt(py).unwrap().call_bound(py, (), Some(&params)).unwrap()
        })
    }

    // For Testing. MAY NOT WORK WITH THE COMPLETED FLAG!
    pub fn _set_skip(&mut self) {
        Python::with_gil(|py| {
            self.opts.bind(py).set_item("skip_download", true).unwrap();
        });
    }

    // Would allow reusing a thread/struct, or some shenanigans.
    pub fn _set_target(&mut self, mut target: String) {
        info!("{} updated target to {}.", self.target, target);
        if target.contains("youtu") && target.len() > 11 {
            target = target.split_off(target.len() - 11);
        }

        self.target = target;
    }

    // Core loop. This is basically a finite state machine with only a couple of core states; it's
    // the "unexpected" handling that adds all the extra complexity.
    pub fn download_loop(&mut self) {
        while !self.complete {
            match Python::with_gil(|py| {
                self.yt_dlp.call_method_bound(py, "download", (&self.target,), None)
            }) {
                Ok(_res) => {
                    info!("{}: Download attempt ended without error.", self.target);
                    self.post_check();
                }
                Err(err) => {
                    if Python::with_gil(|py| -> bool {
                        //TODO: Check for more precise error types.
                        err.is_instance_bound(py, self.yt_error.bind(py))
                    }) {
                        self.error_check(err)
                    } else {
                        error!("{}: Download attempt encountered an unexpected error: {}", self.target, err);
                        self.complete = true;
                    }
                }
            }
        }
    }

    fn error_check(&mut self, err: PyErr) {
        // These are YouTube errors.
        // Make the various errors as enums?
        // These are the ones that seem relevant, albeit not necessarily often
        // UnsupportedError (unsupported url, shouldn't occur normally)
        // GeoRestrictedError (very rare for expected use case)
        // UserNotLive (Not clear if this is the error yt uses for this situation)
        // DownloadError (default error?)
        let temp = err.to_string();
        let mut err_msg = temp.rsplit(' ');
        // ends_with would be nice, but we need the preceding value as well
        match err_msg.next().unwrap() {
            "moments." | "shortly" => {
                // Should be starting soon, retry often.
                // In the future, may want to have this value start to increase after a certain period,
                // for circumstances where the streamer leaves the stream in limbo.
                thread::sleep(time::Duration::from_secs(15));
            }
            "minutes." | "minutes" => {
                // Try again in half the duration or 5 minutes, whatever is sooner, in case the
                // streamer starts early/moves the time forward.
                info!("{}: {}", self.target, err);
                thread::sleep(time::Duration::from_secs(
                    cmp::min(err_msg.next().unwrap().parse::<u64>().unwrap() * 30, 300)));
            }
            "hours." | "hours" => {
                // Try again in an hour. Hour based moves are probably the most common time change,
                // so this minimizes chance of missing a start time change without adding many calls.
                info!("{}: {}", self.target, err);
                thread::sleep(time::Duration::from_secs(60 * 60));
            }
            "days." | "days" => {
                // Try again in 6 hours. Most streams aren't set this far in advanced unless it's
                // big enough the time is fairly set, but we're covering bases.
                info!("{}: {}", self.target, err);
                thread::sleep(time::Duration::from_secs(60 * 60 * 6));
            }
            "years." | "years" => {
                // Generally just chat rooms, may not be worth even trying to continue. Checking once
                // a day, just in case.
                info!("{}: {}", self.target, err);
                thread::sleep(time::Duration::from_secs(60 * 60 * 24));
            }
            // Member video
            // TODO: Add browser cookie support/check
            "perks." => {
                warn!("{}: {}", self.target, err);
                Python::with_gil(|py| {
                    if !self.opts.bind(py).contains("cookiefile").unwrap() {
                        self.opts.bind(py).set_item("cookiefile", "resources/cookies.txt").unwrap()
                    } else {
                        warn!("{}: Failed membership authentication.", self.target);
                        self.complete = true
                    }
                })
            }
            "difficulties." | "difficulties" => {
                // Found when a stream is offline, after having started. May be used elsewhere.
                warn!("{}: {}", self.target, err);
                thread::sleep(time::Duration::from_secs(15));
            }
            val => {
                //Unknown error message.
                error!("{}: Unsupported error message: {} (keyword: {})", self.target, err, val);
                self.complete = true
            }
        }
    }

    // Called after yt-dlp "successfully" returns. Ensure the video is actually done, or sets things
    // to try again/continue.
    fn post_check(&mut self) {
        // YouTube value, most common
        if Python::with_gil(|py| self.hook_struct.borrow(py).yt_bool) {
            // Check YouTube API if video is still live.
            // If the info_dict is accurate, this is a wasted API call, but I don't trust it for the
            // specific situations this is meant to catch.
            // Realistically, this should never be a problem, but it's possible some bad loop could
            // result in exhausting the API quota.
            info!("{}: calling Google API.", self.target);

            // The crate used to call the Google API *requires* a tokio runtime for internal async
            // calls, but it's the only place were async is used and async and threads aren't friends,
            // so we create an abomination of a closure, which requires cloning the target for
            // closure borrow rules. If there's any functionality refactoring in the future, this is
            // near the top of the hit list.
            let temp_target = self.target.clone();
            let response = Runtime::new().unwrap()
                .block_on(async {
                    task::spawn_blocking(|| {
                        api_handler::google_api(temp_target)
                    }).await.unwrap().await
                });

            match response {
                Ok(res) => {
                    match res.snippet.unwrap().live_broadcast_content.unwrap().as_str() {
                        "live" => {
                            //Video still going, continue downloading
                            warn!("{}: Video is still live.", self.target)
                        }
                        "none" => {
                            info!("{}: Video is no longer live.", self.target);
                            self.complete = true;
                        }
                        "upcoming" => {
                            // Not sure if this is even reachable given current conditions.
                            error!("{}: Download attempt successfully finished, but is not live yet!", self.target)
                        }
                        err => {
                            error!("{}: Google API returned an unexpected value for is_live: {}", self.target, err);
                            self.complete = true;
                        }
                    }
                }
                Err(err) => {
                    // Ostensibly there's a problem, potentially the video was removed?
                    error!("{}: Error calling Google API: {}", self.target, err);
                    self.complete = true;
                }
            }
        } else {
            // Other sites
            Python::with_gil(|py| {
                if !self.opts.bind(py).contains("match_filter").unwrap() {
                    self.set_live_only()
                } else if Python::with_gil(|py| self.hook_struct.borrow(py).was_live) {
                    // I don't think this is ever actually reached, but there was a very specific
                    // set of circumstances where a YouTube video would enter an endless failing loop.
                    // This would prevent that if YouTube wasn't checked separately via API, and
                    // hopefully stops any similar loops from other sources.
                    error!("{}: Reached the non-YouTube post-check with a is_live filter and a was_live status.", self.target);
                    self.complete = true
                }
            })
        }
    }
}
