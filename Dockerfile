FROM alpine

WORKDIR /app
EXPOSE 80
ENV ROCKET_PORT 80
ENV ROCKET_ADDRESS 0.0.0.0
ENV GTFS_DIR /volume/gtfs
COPY target/x86_64-unknown-linux-musl/release/webserver_svg transit-radar
COPY VBB_Colours.csv ./
ENTRYPOINT ["/app/transit-radar"]
