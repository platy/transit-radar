# Task breakdown

## Backend

[x] Include GTFS files
[x] Open and parse some relevant GTFS table
[x] Show stops, frequencies and travel times along the U8
[x] Create station groups
[x] Show stops and travel times on a route from a point in the middle (including different destinations)
[x] Ignore trips to stops which can be reached faster
[x] Branch travel times from a stop including estimate for transfer times
[x] Start from a parent station
[x] Exclude any stops on trips which wont get somewhere faster in the future
[x] Load routes etc to show better info when plotting the journeys
[x] Calculate bearings for stops
[x] Write algorithm
[x] Produce output json
[x] Search a station by name
[x] Try different start stations
[x] Test loading a larger cache time period
[x] Make a day filter option in the data load
[x] Make web service using old Sunday 7pm data
[x] Build new multi day cache
[x] Station search & selection
[x] Add only stops with a connection to the search
[x] Multi word station search
[x] Bug: failing to transfer to U2 from Alex bhf or from hauptbahnhof tief to upstairs
[x] Error: to go northbound on S2 it's currently suggesting going south to Humbolthain and then changing to north - i guess its not any slower than waiting for that train at humbolthain, need to optimize for less changes
[x] S85 trip is disjointed on earlier stops while it is slow between arrival times and departure times, as these are slow, it should just use departure times
[x] Eberswalderstrasse u2 southbound from schönhauser allee arrives 30 seconds after the northbound from senefelderplatz, as the transfer takes 3 mins between the platforms they are both the earliest arrivals at their respective stations. We only want to emit the earliest arrival at a station, but for the search we need the earliest arrival at each stop
[x] Filter out transfers to stations with no trips
[x] Time selection
[x] Day filter in search to enable use of multi day cache
[x] Day selection
[x] Handle the D_ stopids in the gtfs data
[] Reload GTFS data each day
[] Get the timetable publish date from the calendar.txt
[] Tool to filter GTFS data for development so that cache is not needed
[] Remove the cache
[] Allow connections to several trips of the same station and route (eg different directions), currently it is filtered to one
-
[] CLI tool to lookup departures for debugging
-
[] Find stops within a distance of a point sorted by distance
[] Start from spot between stations
[] Build a graph of average times

# Performance

[x] Improve Time deserialisation, no parsing to string or splitting ~ 25%
[x] Remove duplication of trips read ~ 4%
[x] Parse Time as byte array as checking the char boundaries is slow and unnecessary maybe 6%
[] Parallelize stoptime deserialisation and reading, reading is 30% and deserialisation is 50%
[] Buffer reads from the cache and writes to it

## Code 
[x]StopId should not be String, maybe &str / str?
[x] Modularise the stoptime reader into a struct by understanding the lifetimes (had to use RefCell)
[x] serialise the data so that i can have faster iteration
[x] Switch to id_arena - it will serialise easier and I won't need all these borrows complicating things
[x] Build and deploy scripts
[] Macro for compile time Time literals
[] Organise all the parsing and data lookups
[] use debugging
[] improve the error reporting in the parser
[] remove the Strings
[] save images and start writing blog post about the development

## Frontend

[x] Use json?
[x] Draw stop points
[x] Connect with lines
[x] Add station names
[x] Use route colours
[x] Add some filtering / highlighting
[x] show wait times / transfers together and distinct from travel
[x] Have the initial wait go to the left to not run over the station name
[x] Draw connections in reverse order to have the first / shorter ones on top
[x] Connect with curves
[x] Trip start control point to reduce curve into the origin
[x] Start bearing toward next stop to avoid curve into the origin
[] Add text description to svg
[] Save button
[] Add key with emphasis highlighting
[] Show station names on hover

# Both

[] Filters for buses, trams, etc
[] Shareable Uris
[] Start from coords
[] Switch to times from seconds
[] Smooth animation
  [] hack - how long is first departure from origin - load that much extra data and then just rerender until that departure and then reload data
  [] hack+ - on reload do a complete search in the backend but just send a diff. We may add some new stations and change the earliest arrival time at some other stations. Some trips are removed and added, and some stops on trips which were not shown will be shown. Calculate placement of transfers in frontend
  [] lower the backend calculations by re searching over the existing tree rather than from scratch after a missed departure. This will massively complicate the algorithm and structure and i can't really justify it now.

# Intermediate data format

```
{
  locations[locationId]: {
    name: string,
    bearing: number,
  },
  connections[]: {
    from: StopID | "origin", // origin refers to locationId 0
    to: StopID | Location,
    walkingTime,
  },
  routes[routeID]: {
    name: string,
    colour,
    type: string / enum,
    stops[stopNumber]: {
      locationID,
      timeFromHome,
      isClosest: boolean
    }
  }
}
type StopId = {
  routeId,
  stopNumber,
}

# Deploying

Cross build for linux:
```
cargo build --release --target x86_64-unknown-linux-musl  
```

Build frontend:
```
cd frontend
npm run build
```

Deploy backend change:
```
ssh root@s4.njk.onl /app/transit-radar/service.sh stop
scp target/x86_64-unknown-linux-musl/release/transit-radar root@s4.njk.onl:/app/transit-radar/ 
ssh root@s4.njk.onl /app/transit-radar/service.sh start
```

Deploy frontend change:
```
scp -r frontend/build root@s4.njk.onl:/app/transit-radar/ 
```

Deploy everything:
```
scp -r target/x86_64-unknown-linux-musl/release/transit-radar gtfs/cache-fri-19:00:00-19:30:00 frontend/build run.sh service.sh root@s4.njk.onl:/app/transit-radar/ 
```

Start
```
ssh root@s4.njk.onl /app/transit-radar/service.sh start
```

Stop
```
ssh root@s4.njk.onl /app/transit-radar/service.sh stop
```

Restart
```
ssh root@s4.njk.onl /app/transit-radar/service.sh restart
```

Logs 
```
ssh root@s4.njk.onl tail -f /var/log/transit-radar/log
```
