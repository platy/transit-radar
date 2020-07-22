FROM alpine

WORKDIR /app
EXPOSE 80
ENV PORT 80
ENV STATIC_DIR /app/www
ENV GTFS_DIR /volume/gtfs
COPY target/x86_64-unknown-linux-musl/release/webserver_sync transit-radar
COPY VBB_Colours.csv ./
COPY seed-frontend/index.html seed-frontend/storybook.html www/
COPY seed-frontend/pkg www/pkg
COPY seed-frontend/static www/static
CMD /app/transit-radar
