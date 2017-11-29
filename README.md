# spotify-connect-scrobbler
[![Build Status](https://travis-ci.org/bobbo/spotify-connect-scrobbler.svg?branch=master)](https://travis-ci.org/bobbo/spotify-connect-scrobbler)

*spotify-connect-scrobbler* is a Last.fm music logging ("scrobbling") service for Spotify. It uses [Spotify Connect](https://www.spotify.com/connect/) to allow you to  log music played on any Spotify device, including those which do not have any Last.fm support (such as Amazon Echo).

# Usage

To use *spotify-connect-scrobbler* have your Spotify username & password, your Last.fm username & password, plus a [Last.fm API key and API secret](https://www.last.fm/api/account/create) to hand.

Clone the repo,have Rust installed (`v1.18` minimum required) build with Cargo (`cargo build`). Then run:

`./target/debug/spotify-connect-scrobbler --spotify-username <Spotify username> --spotify-password <Spotify password> --lastfm-username <Last.fm username> --lastfm-password <Last.fm password> --lastfm-api-key <Last.fm API key> --lastfm-api-secret <Last.fm API secret>`

The service will sit in the background and log all Spotify tracks played from any Connect enabled client to the given Last.fm account. It is strongly recommended that you turn off Last.fm integration in any Spotify client where it is enabled (Desktop & Mobile apps). Instructions for the opposite [here](https://support.spotify.com/us/using_spotify/app_integrations/scrobble-to-last-fm/).

#### Other Options

* `--name <Device name>` - Sets the Spotify Connect device name (defaults to 'Scrobbler'), this name is visible in the Spotify Connect device chooser in Spotify clients

# Implementation
 
 *spotify-connect-scrobbler* is built on top (more accurately, is a fork of) of Paul Lietar's [librespot](https://github.com/plietar/librespot) project, an open-source Spotify Connect implementation in Rust. It connects to Spotify as a fully-fledged Spotify Connect device. The active Spotify Connect device (the one playing music) broadcasts its status to all other Connect devices on an account, in order to show now-playing track data on other clients. For example, when playing Spotify tracks on an Amazon Echo, the Echo device will broadcast the currently playing track so that it can be shown on the Spotify app on your phone). Thus *spotify-connect-scrobbler* can see the currently playing track and send that to be logged on your Last.fm account.

 # License

 Released under the MIT license. See `LICENSE`.
 