// TODO: many items from tokio-core::io have been deprecated in favour of tokio-io
#![allow(deprecated)]

#[macro_use] extern crate log;
extern crate env_logger;
extern crate futures;
extern crate getopts;
extern crate librespot;
extern crate tokio_core;
extern crate tokio_signal;

use env_logger::LogBuilder;
use futures::{Future, Async, Poll, Stream};
use std::env;
use std::io::{self, stderr, Write};
use std::path::PathBuf;
use std::process::exit;
use std::str::FromStr;
use tokio_core::reactor::{Handle, Core};
use tokio_core::io::IoStream;
use std::mem;

use librespot::core::authentication::{get_credentials, Credentials};
use librespot::core::cache::Cache;
use librespot::core::config::{DeviceType, SessionConfig, ConnectConfig};
use librespot::core::session::Session;
use librespot::core::version;

use librespot::scrobbler::ScrobblerConfig;
use librespot::spirc::{Spirc, SpircTask};

fn usage(program: &str, opts: &getopts::Options) -> String {
    let brief = format!("Usage: {} [options]", program);
    opts.usage(&brief)
}

fn setup_logging(verbose: bool) {
    let mut builder = LogBuilder::new();
    match env::var("RUST_LOG") {
        Ok(config) => {
            builder.parse(&config);
            builder.init().unwrap();

            if verbose {
                warn!("`--verbose` flag overidden by `RUST_LOG` environment variable");
            }
        }
        Err(_) => {
            if verbose {
                builder.parse("mdns=info,librespot=trace");
            } else {
                builder.parse("mdns=info,librespot=info");
            }
            builder.init().unwrap();
        }
    }
}

struct Setup {
    cache: Option<Cache>,
    session_config: SessionConfig,
    connect_config: ConnectConfig,
    credentials: Option<Credentials>,
    scrobbler_config: ScrobblerConfig
}

fn setup(args: &[String]) -> Setup {
    let mut opts = getopts::Options::new();
    opts.optopt("c", "cache", "Path to a directory where files will be cached.", "CACHE")
        .optopt("n", "name", "Device name (defaults to Scrobbler)", "NAME")
        .optopt("", "device-type", "Displayed device type", "DEVICE_TYPE")
        .optflag("v", "verbose", "Enable verbose output")
        .optopt("", "spotify-username", "Username to sign in with", "USERNAME")
        .optopt("", "spotify-password", "Password", "PASSWORD")
        .optopt("", "lastfm-username", "Last.fm Username", "LASTFM_USERNAME")
        .optopt("", "lastfm-password", "Last.fm Password", "LASTFM_PASSWORD")
        .optopt("", "lastfm-api-key", "Last.fm API Key", "API_KEY")
        .optopt("", "lastfm-api-secret", "Last.fm API Secret", "SECRET");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            writeln!(stderr(), "error: {}\n{}", f.to_string(), usage(&args[0], &opts)).unwrap();
            exit(1);
        }
    };

    let verbose = matches.opt_present("verbose");
    setup_logging(verbose);

    info!("librespot {} ({}). Built on {}. Build ID: {}",
             version::short_sha(),
             version::commit_date(),
             version::short_now(),
             version::build_id());

    let name = matches.opt_str("name").unwrap_or(String::from("Scrobbler"));
    let use_audio_cache = false;

    let cache = matches.opt_str("c").map(|cache_location| {
        Cache::new(PathBuf::from(cache_location), use_audio_cache)
    });

    let cached_credentials = cache.as_ref().and_then(Cache::credentials);
    let credentials = get_credentials(matches.opt_str("spotify-username"),
                                      matches.opt_str("spotify-password"),
                                      cached_credentials);

    let session_config = {
        let device_id = librespot::core::session::device_id(&name);

        SessionConfig {
            user_agent: version::version_string(),
            device_id: device_id,
        }
    };

    let scrobbler_config = ScrobblerConfig {
        api_key: matches.opt_str("lastfm-api-key").expect("Invalid Last.fm API key"),
        api_secret: matches.opt_str("lastfm-api-secret").expect("Invalid Last.fm API secret"),
        username: matches.opt_str("lastfm-username").expect("Invalid Last.fm username"),
        password: matches.opt_str("lastfm-password").expect("Invalid Last.fm password")
    };

    let connect_config = {
        let device_type = matches.opt_str("device-type").as_ref()
            .map(|device_type| DeviceType::from_str(device_type).expect("Invalid device type"))
            .unwrap_or(DeviceType::default());

        ConnectConfig {
            name: name,
            device_type: device_type,
        }
    };

    Setup {
        cache: cache,
        session_config: session_config,
        connect_config: connect_config,
        credentials: credentials,
        scrobbler_config: scrobbler_config
    }
}

struct Main {
    cache: Option<Cache>,
    session_config: SessionConfig,
    connect_config: ConnectConfig,
    handle: Handle,

    signal: IoStream<()>,

    spirc: Option<Spirc>,
    spirc_task: Option<SpircTask>,
    connect: Box<Future<Item=Session, Error=io::Error>>,

    scrobbler_config: ScrobblerConfig,

    shutdown: bool,
}

impl Main {
    fn new(handle: Handle, setup: Setup) -> Main {
        let mut task = Main {
            handle: handle.clone(),
            cache: setup.cache,
            session_config: setup.session_config,
            connect_config: setup.connect_config,

            connect: Box::new(futures::future::empty()),
            spirc: None,
            spirc_task: None,
            shutdown: false,
            signal: tokio_signal::ctrl_c(&handle).flatten_stream().boxed(),
            scrobbler_config: setup.scrobbler_config
        };

        if let Some(credentials) = setup.credentials {
            task.credentials(credentials);
        }

        task
    }

    fn credentials(&mut self, credentials: Credentials) {
        let config = self.session_config.clone();
        let handle = self.handle.clone();

        let connection = Session::connect(config, credentials, self.cache.clone(), handle);

        self.connect = connection;
        self.spirc = None;
        let task = mem::replace(&mut self.spirc_task, None);
        if let Some(task) = task {
            self.handle.spawn(task);
        }
    }
}

impl Future for Main {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        loop {
            let mut progress = false;

            if let Async::Ready(session) = self.connect.poll().unwrap() {
                self.connect = Box::new(futures::future::empty());
                let connect_config = self.connect_config.clone();

                let (spirc, spirc_task) = Spirc::new(connect_config, session, self.scrobbler_config.clone());
                self.spirc = Some(spirc);
                self.spirc_task = Some(spirc_task);

                progress = true;
            }

            if let Async::Ready(Some(())) = self.signal.poll().unwrap() {
                if !self.shutdown {
                    if let Some(ref spirc) = self.spirc {
                        spirc.shutdown();
                    }
                    self.shutdown = true;
                } else {
                    return Ok(Async::Ready(()));
                }

                progress = true;
            }

            if let Some(ref mut spirc_task) = self.spirc_task {
                if let Async::Ready(()) = spirc_task.poll().unwrap() {
                    if self.shutdown {
                        return Ok(Async::Ready(()));
                    } else {
                        panic!("Spirc shut down unexpectedly");
                    }
                }
            }

            if !progress {
                return Ok(Async::NotReady);
            }
        }
    }
}

fn main() {
    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let args: Vec<String> = std::env::args().collect();

    core.run(Main::new(handle, setup(&args))).unwrap()
}
