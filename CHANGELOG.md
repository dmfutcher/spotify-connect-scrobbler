Version 0.1.2 - 2017-11-05
==========================

 * Replace StdoutSink with NilSink, stops binary music data being piped
   to the terminal's stdout. (#9)
 * Remove `--backend` flag, will always be set to NilSink (#4)

Version 0.1.1 - 2017-10-04
==========================

  * Add album name info to track scrobbles (#2)
  * Scrobbling triggered on start of new track, fixes tracks failing to 
    scrobble under certain conditions (#3)
  * Make `--name` flag optional; defaults to 'Scrobbler'
  * Significant internal refactoring, including removal of unnecessary
    `unsafe` code.

Version 0.1.0 - 2017-08-26
==========================

  * Initial release
