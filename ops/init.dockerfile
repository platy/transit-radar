FROM alpine:latest
RUN apk add --update curl && rm -rf /var/cache/apk/*

COPY init.sh /
ENV GTFS_PARENT=/volume
CMD /init.sh $GTFS_PARENT
