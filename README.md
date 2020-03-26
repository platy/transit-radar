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
[] Search a station by name
[] Try different start stations
[] try different days / times
[] Test loading a larger cache time period
[] Make web service
[] Build svg in backend
[] CLI tool to lookup departures for debugging eg. below
[] Error: to go northbound on S2 it's currently suggesting going south to Humbolthain and then changing to north - i guess its not any slower than waiting for that train at humbolthain, need to optimize for less changes
-
[] Find stops within a distance of a point sorted by distance
[] Start from spot between stations
[] Build a graph of average times

## Code 
[x]StopId should not be String, maybe &str / str?
[x] Modularise the stoptime reader into a struct by understanding the lifetimes (had to use RefCell)
[x] serialise the data so that i can have faster iteration
[] Switch to id_arena - it will serialise easier and I won't need all these borrows complicating things
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
-
[] Add key with emphasis highlighting
[] Connect with curves
[] Heuristic choice of start bearing to reduce curve into the origin
[] Show / hide station names / show on hover?
[] get geo position for search

## Stretch

[] Choose start point
[] Find start point with GPS
[] Choose which lines to include
[] Offer data update when available
[] Cache all needed data offline

# Algorithm & Data structure

Pre steps involve loading filtered data into maps and vecs for fast lookup

prioritized queue sorted in order of arrival time containing:
* stop times (representing being on a particular trip at a particular station at a particular time)
* connections ( making a connection between stops or trips in a particular amount of time)

(multi)map of stop ids to their nodes in the graph ordered by arrival time

graph of possible journeys

For each element taken from the queue:
if it is a stop time: If the arrival for that stop is not the fastest, nothing to do, otherwise:
1. lookup connections for the arrival stop and add to the queue if they are within the time limit
2. add entry for stop to graph and into/replace stop map
if it is a connection: If the arrival for that stop is not the fastest, nothing to do, otherwise:
1. lookup stoptimes departing from the arrival stop in a reasonable time period and add their remaining trips to the queue
2. add entry for stop to graph and into/replace stop map

Edge cases:
* if stop arrivals are very similar (within 1 minute) we would want to merge them together rather than discarding the slower one - so maybe we don't include just the fastest node in the stops map but maybe the sorted collection of stop arrivals so that the view layer can make these decisions


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
