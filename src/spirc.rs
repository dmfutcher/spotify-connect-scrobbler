use futures::sink::BoxSink;
use futures::stream::BoxStream;
use futures::sync::mpsc;
use futures::{Future, Stream, Sink, Async, Poll};
use protobuf::{self, Message};

use core::config::ConnectConfig;
use core::mercury::MercuryError;
use scrobbler::{Scrobbler, ScrobblerConfig};
use core::session::Session;
use core::util::{now_ms, SpotifyId, SeqGenerator};
use core::version;

use protocol;
use protocol::spirc::{PlayStatus, State, MessageType, Frame, DeviceState};


pub struct SpircTask {
    sequence: SeqGenerator<u32>,

    ident: String,
    device: DeviceState,
    state: State,

    subscription: BoxStream<Frame, MercuryError>,
    sender: BoxSink<Frame, MercuryError>,
    commands: mpsc::UnboundedReceiver<SpircCommand>,

    shutdown: bool,
    session: Session,

    scrobbler: Scrobbler
}

pub enum SpircCommand {
    Shutdown
}

pub struct Spirc {
    commands: mpsc::UnboundedSender<SpircCommand>,
}

fn initial_state() -> State {
    protobuf_init!(protocol::spirc::State::new(), {
        repeat: false,
        shuffle: false,
        status: PlayStatus::kPlayStatusStop,
        position_ms: 0,
        position_measured_at: 0,
    })
}

fn initial_device_state(config: ConnectConfig, volume: u16) -> DeviceState {
    protobuf_init!(DeviceState::new(), {
        sw_version: version::version_string(),
        is_active: false,
        can_play: true,
        volume: volume as u32,
        name: config.name,
        capabilities => [
            @{
                typ: protocol::spirc::CapabilityType::kCanBePlayer,
                intValue => [1]
            },
            @{
                typ: protocol::spirc::CapabilityType::kDeviceType,
                intValue => [config.device_type as i64]
            },
            @{
                typ: protocol::spirc::CapabilityType::kGaiaEqConnectId,
                intValue => [1]
            },
            @{
                typ: protocol::spirc::CapabilityType::kSupportsLogout,
                intValue => [0]
            },
            @{
                typ: protocol::spirc::CapabilityType::kIsObservable,
                intValue => [1]
            },
            @{
                typ: protocol::spirc::CapabilityType::kVolumeSteps,
                intValue => [64]
            },
            @{
                typ: protocol::spirc::CapabilityType::kSupportedContexts,
                stringValue => [
                    "album",
                    "playlist",
                    "search",
                    "inbox",
                    "toplist",
                    "starred",
                    "publishedstarred",
                    "track",
                ]
            },
            @{
                typ: protocol::spirc::CapabilityType::kSupportedTypes,
                stringValue => [
                    "audio/local",
                    "audio/track",
                    "local",
                    "track",
                ]
            }
        ],
    })
}

impl Spirc {
    pub fn new(config: ConnectConfig, session: Session, scrobbler_config: ScrobblerConfig)
        -> (Spirc, SpircTask)
    {
        debug!("new Spirc[{}]", session.session_id());

        let ident = session.device_id().to_owned();

        let uri = format!("hm://remote/3/user/{}/", session.username());

        let subscription = session.mercury().subscribe(&uri as &str);
        let subscription = subscription.map(|stream| stream.map_err(|_| MercuryError)).flatten_stream();
        let subscription = subscription.map(|response| -> Frame {
            let data = response.payload.first().unwrap();
            protobuf::parse_from_bytes(data).unwrap()
        }).boxed();

        let sender = Box::new(session.mercury().sender(uri).with(|frame: Frame| {
            Ok(frame.write_to_bytes().unwrap())
        }));

        let (cmd_tx, cmd_rx) = mpsc::unbounded();

        let volume = 0xFFFF;
        let device = initial_device_state(config, volume);

        let scrobbler = Scrobbler::new(scrobbler_config, session.clone());

        let mut task = SpircTask {
            sequence: SeqGenerator::new(1),

            ident: ident,

            device: device,
            state: initial_state(),

            subscription: subscription,
            sender: sender,
            commands: cmd_rx,

            shutdown: false,
            session: session.clone(),

            scrobbler: scrobbler
        };

        let spirc = Spirc {
            commands: cmd_tx,
        };

        task.hello();

        (spirc, task)
    }

    pub fn shutdown(&self) {
        let _ = mpsc::UnboundedSender::send(&self.commands, SpircCommand::Shutdown);
    }
}

impl Future for SpircTask {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        loop {
            let mut progress = false;

            if !self.shutdown {
                match self.subscription.poll().unwrap() {
                    Async::Ready(Some(frame)) => {
                        progress = true;
                        self.handle_frame(frame);
                    }
                    Async::Ready(None) => panic!("subscription terminated"),
                    Async::NotReady => (),
                }

                match self.commands.poll().unwrap() {
                    Async::Ready(Some(command)) => {
                        progress = true;
                        self.handle_command(command);
                    }
                    Async::Ready(None) => (),
                    Async::NotReady => (),
                }

                match self.scrobbler.poll() {
                    Ok(Async::Ready(_)) => {
                        progress = true;
                    },
                    Ok(Async::NotReady) => {

                    },
                    Err(err) => {
                        error!("Scrobbler error: {:?}", err);
                    }
                }
            }

            let poll_sender = self.sender.poll_complete().unwrap();

            // Only shutdown once we've flushed out all our messages
            if self.shutdown && poll_sender.is_ready() {
                return Ok(Async::Ready(()));
            }

            if !progress {
                return Ok(Async::NotReady);
            }
        }
    }
}

impl SpircTask {
    fn handle_command(&mut self, cmd: SpircCommand) {
        match cmd {
            SpircCommand::Shutdown => {
                CommandSender::new(self, MessageType::kMessageTypeGoodbye).send();
                self.shutdown = true;
                self.commands.close();
            }
        }
    }

    fn handle_frame(&mut self, frame: Frame) {
        debug!("{:?} {:?} {} {} {}",
               frame.get_typ(),
               frame.get_device_state().get_name(),
               frame.get_ident(),
               frame.get_seq_nr(),
               frame.get_state_update_id());

        if frame.get_ident() == self.ident ||
           (frame.get_recipient().len() > 0 && !frame.get_recipient().contains(&self.ident)) {
            return;
        }

        match frame.get_typ() {
            MessageType::kMessageTypeHello => {
                self.notify(Some(frame.get_ident()));
            }

            MessageType::kMessageTypeNotify => {
                // Inactive devices won't be playing anything, so we don't need to scrobble it
                if !frame.get_device_state().get_is_active() {
                    return ();
                }

                //println!("{:?}", frame);
                //println!("Type: {:?}", frame.get_typ());
                let state = frame.get_state();
                let playing_index = state.get_playing_track_index();
                let tracks = state.get_track();
                if tracks.len() > 0 {
                    let playing_track_ref = state.get_track()[playing_index as usize].clone();
                    let playing_track_spotify_id = SpotifyId::from_raw(playing_track_ref.get_gid());

                    self.scrobbler.update_current_track(playing_track_spotify_id);
                    info!("Relevant SPIRC frame; Current track Spotify ID: {:?}", playing_track_spotify_id);
                }
                
            }
            _ => (),
        }
    }

    fn hello(&mut self) {
        CommandSender::new(self, MessageType::kMessageTypeHello).send();
    }

    fn notify(&mut self, recipient: Option<&str>) {
        let mut cs = CommandSender::new(self, MessageType::kMessageTypeNotify);
        if let Some(s) = recipient {
            cs = cs.recipient(&s);
        }
        cs.send();
    }
}

impl Drop for SpircTask {
    fn drop(&mut self) {
        debug!("drop Spirc[{}]", self.session.session_id());
    }
}

struct CommandSender<'a> {
    spirc: &'a mut SpircTask,
    frame: protocol::spirc::Frame,
}

impl<'a> CommandSender<'a> {
    fn new(spirc: &'a mut SpircTask, cmd: MessageType) -> CommandSender {
        let frame = protobuf_init!(protocol::spirc::Frame::new(), {
            version: 1,
            protocol_version: "2.0.0",
            ident: spirc.ident.clone(),
            seq_nr: spirc.sequence.get(),
            typ: cmd,

            device_state: spirc.device.clone(),
            state_update_id: now_ms(),
        });

        CommandSender {
            spirc: spirc,
            frame: frame,
        }
    }

    fn recipient(mut self, recipient: &'a str) -> CommandSender {
        self.frame.mut_recipient().push(recipient.to_owned());
        self
    }

    #[allow(dead_code)]
    fn state(mut self, state: protocol::spirc::State) -> CommandSender<'a> {
        self.frame.set_state(state);
        self
    }

    fn send(mut self) {
        if !self.frame.has_state() && self.spirc.device.get_is_active() {
            self.frame.set_state(self.spirc.state.clone());
        }

        let send = self.spirc.sender.start_send(self.frame).unwrap();
        assert!(send.is_ready());
    }
}
