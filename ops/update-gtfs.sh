#! /bin/sh
set -e

cd "$1"
echo Getting timetables
curl https://www.vbb.de/fileadmin/user_upload/VBB/Dokumente/API-Datensaetze/gtfs-mastscharf/GTFS.zip --output gtfs.zip
echo Unzipping
mkdir -p gtfs
unzip -of -d gtfs gtfs.zip
echo Updated

