#! /bin/sh

if [[ ! -d $1/gtfs ]]
then
  echo No Timetables, initialising
  cd $1
  echo Getting timetables
  curl https://www.vbb.de/fileadmin/user_upload/VBB/Dokumente/API-Datensaetze/GTFS.zip --output gtfs.zip
  echo Unzipping
  mkdir gtfs
  unzip gtfs.zip -d gtfs
  echo Initialised
else
  echo Timetables already present
fi
