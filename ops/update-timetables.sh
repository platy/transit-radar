#!/bin/bash

cd /app/transit-radar
rm -rf gtfs.old
mv gtfs gtfs.old
curl https://www.vbb.de/media/download/2029 --output gtfs.zip
unzip gtfs.zip -d gtfs
/app/transit-radar/service.sh restart
