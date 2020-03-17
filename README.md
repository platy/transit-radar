# Task breakdown

## Backend

[x] Include GTFS files
[x] Open and parse some relevant GTFS table
[x] Show stops, frequencies and travel times along the U8
[] Show stops and travel times on a route from a point in the middle (including different destinations)
[] Branch travel times from a stop including estimate for transfer times
[] Find stops within a distance of a point sorted by distance
[] Write algorithm
[] Produce output json
[] Make web service / run fully in wasm?

## Frontend

[] Use json
[] Draw stop points
[] Connect with lines
[] Connect with curves
[] Add station names
[] Add transfers
[] Show / hide station names / show on hover?

## Stretch

[] Choose start point
[] Find start point with GPS
[] Choose which lines to include
[] Offer data update when available
[] Cache all needed data offline


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