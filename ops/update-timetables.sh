#!/bin/sh

cd $1
rm -rf gtfs.old
mv gtfs gtfs.old
curl https://www.vbb.de/media/download/2029 --output gtfs.zip
unzip gtfs.zip -d gtfs
