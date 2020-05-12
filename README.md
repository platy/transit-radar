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
[x] Get the timetable publish date from the calendar.txt
[x] Filter GTFS data for development so that cache is not needed
[x] Remove the cache
[x] Allow connections to several trips of the same station and route (eg different directions), currently it is filtered to one
[x] Replace arena with a map of trips
[] Station search more forgiving with umlauts etc. maybe find crate to build a linguistic index / search map
[] Reload GTFS data each day
[] Count + number clients based on IP address + log anonymised when a new client connects
[] Sub count clients by hash of user agent
[] Record logging session per client and log statistical info every minute, every hour and every day + immediate info if enabled by env var
[] Identify regions for the clients by IP address
[] Use exceptions in calendar_dates.txt
[] Handle day overlap
-
[] read from zip
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
[] Csv reader might be faster with ByteRecord enabling read of borrowed data [https://docs.rs/csv/1.1.3/csv/struct.ByteRecord.html]
[] See if csv reader can be sped up

## Code 
[x] StopId should not be String, maybe &str / str?
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
[] Rewrite in rust/wasm
[] Add text description to svg
[] Save button
[] Add key with emphasis highlighting
[] Show station names on hover

# Both

[x] Filters for buses, trams, etc
[] Shareable Uris
[] Start from coords
[] Incoming radar
[] Show only important stations to reduce the number shown
[] Average times
[] London
[] Switch to times from seconds
[] Choose times other than now
[] Smooth animation
  [] hack - how long is first departure from origin - load that much extra data and then just rerender until that departure and then reload data
  [] hack+ - on reload do a complete search in the backend but just send a diff. We may add some new stations and change the earliest arrival time at some other stations. Some trips are removed and added, and some stops on trips which were not shown will be shown. Calculate placement of transfers in frontend
  [] lower the backend calculations by re searching over the existing tree rather than from scratch after a missed departure. This will massively complicate the algorithm and structure and i can't really justify it now.
[] Search up to an hour

# Rust/canvas frontend

[x] Load data in browser
[x] Draw routes
[x] Add frontend controls
[x] Draw grid
[x] Support 2x scaling for retina
[x] Draw connections
[x] Colour routes
[x] Style routes
[x] Redraw without search until a train departs
[x] Route curves
[x] Get data from backend
[x] Pre-search data filtering in backend
[x] Data diffing using session; or
[x] Search from known data while waiting for backend
[] Cleanup - todos, extractions, refactor, hardcoded stuff, edge cases, names, docs, things below
[x] Sync count check and synchronisation and request debounce in client -(with timeout)-
[] Stations
[] Smooth animation by using millisecond precision for start time; or only animate when the second changes and save cpu (checkbox?)
[] Search an extra amount (maximum will be the time to first departure) and then set expiry to that and filter when drawing
[] Add station search
[] fast mode
[] Load data before its needed
[] Colour properly
[] Draw in proper order
--
[] Decent time sync between front and back (backend responsible for macro time and frontend for micro - effectively just an offset form the frontend time)
[] Get time initially from backend
[] Make sure it doesn't animate when not visible to save cpu
[] Don't freeze display thread while deserialising
[] Reduce size of wasm
[] Fast load with first image from backend
[] Animate search change
[] Geometry parameters and animation
[] Break up initial load into smaller parts to show something quicker
[] Change time
[] Click station to show from there (from arrival time or now?)

# Models

1. GTFS data
2. Indexed data for searching
3. Search algorithm temporary
4. Search output
5. Frontend model transferred to client
6. SVG model

## New frontend

To redesign the frontend for lower bandwidth, smooth animation, lower cpu usage, the potential for backend performance improvements and interactivity on the frontend.

5. Filtered model of just what the frontend needs transferred to client.
  This would have a reduced amount of duplicated data, possibly by have a stateful session so the backend knows what the client already has, or better if the client could represent it's existing state to the backend (it's better for scaling but it probably wont manage the performance).
  Taking lessons from graphql, the client should be in charge of what info it needs
6. Frontend data model (possibly also stored on backend to diff against) the model may be the same as the backend search data, just filtered to what is relevant
7. Do we then run the search again on the frontend data that has been synced? Can it use the same implementation?
8. Search result to display
9. Display model which can be updated interactively and animate transitions without searching again. This model should be fast to draw and run on the render thread (60fps hopefully)

Changes to do for this:
[] Make the search model and algorithm able to be used on the frontend as well - not depending on GTFS stuff, serializable, extracted to it's own crate etc.
[] Use search algorithm on backend to produce filtered search data to be used by the same algorithm.
[] Use search algorithm on frontend
[] Build a 2 stage renderer 1 converts the search result into a sort of DOM and accepts display parameters (and can be switched) and another that renders the DOM to canvas, and implements transitions
[] Diff filtered data on backend and sync to frontend

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
scp target/x86_64-unknown-linux-musl/release/webserver root@s4.njk.onl:/app/transit-radar/transit-radar
ssh root@s4.njk.onl /app/transit-radar/service.sh start
```

Deploy frontend change:
```
scp -r frontend/build root@s4.njk.onl:/app/transit-radar/ 
```

Deploy everything:
```
scp -r target/x86_64-unknown-linux-musl/release/transit-radar frontend/build run.sh service.sh update-timetables.sh root@s4.njk.onl:/app/transit-radar/ 
```

Update timetables:
```
ssh root@s4.njk.onl /app/transit-radar/update-timetables.sh
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
