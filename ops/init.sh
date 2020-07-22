#! /bin/sh

if [[ ! -d $1/gtfs ]]
then
  echo No Timetables, initialising
  cd $1
  echo Getting timetables
  curl https://www.vbb.de/media/download/2029 --output gtfs.zip
  echo Unzipping
  mkdir gtfs
  unzip gtfs.zip -d gtfs
  echo Initialised
else
  echo Timetables already present
fi
