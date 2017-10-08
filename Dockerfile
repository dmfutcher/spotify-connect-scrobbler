FROM rust

COPY . /tmp
WORKDIR /tmp
RUN cargo build --release

CMD   "./target/release/spotify-connect-scrobbler" "--spotify-username" "${SPOTIFY_USERNAME}" "--spotify-password" "${SPOTIFY_PASSWORD}" \
      "--lastfm-username" "${LASTFM_USERNAME}" "--lastfm-password" "${LASTFM_PASSWORD}" "--lastfm-api-key" "${LASTFM_API_KEY}" "--lastfm-api-secret" "${LASTFM_API_SECRET}" 