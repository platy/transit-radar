# Building the transit-radar

## Parsing the GTFS csvs (& publishing a crate?)

VBB provides all it's timetables in the GTFS format which was created by google so that google maps could give transit directions.

I need to make first a bunch of structs for the GTFS csv formats and implement serde's Deserialize for them.

[include ER - diagram]
[explain all the parts that we need]

I included the time library but then found that times in the data have a quirk - they can go up to about 25 hours, this is presumably as the transit day ends at 3am here in Berlin. We want to maintain the data in this way, otherwise things will be quite complicated. So we implement our own Time struct and Deserialize for it.

RouteID has this format which means that the easiest thing to deserialize to is a string, but string is kind of awkward and it is unique in the first part so we make a custom serialize there too.

## Streaming from StopTimes

stop_time.csv is very large, but we don't need all the data when we are routing in a particular time frame, so we can filter it at load time

## Building the graph data to do lookup on

The graph that we can produce is a directed graph where the edges can also have a temporal validity which is relevant for the search. 

why? - it is helpful to add all the stops of a trip so I produce a slice of stoptimes?

## Doing the lookup

## using an arena

switched to vec as its easier / quicker to serialise

## Serialising cache

## Warp

Arc needed to share the (immutable) db between threads

## Cross compiling

[https://timryan.org/2018/07/27/cross-compiling-linux-binaries-from-macos.html]

At this point i want to deploy, I am developing on MacOS and my server is gnu/linux so I need to cross compile. The cosystem has built in suport for cross compiling.

First I install a linux target using rustup:
rustup target add x86_64-unknown-linux-musl

Then I also need a linux linker (otherwise there are some mysterious linker errors), and to tell the compiler to use it for this target:
brew install FiloSottile/musl-cross/musl-cross

Then i can compile:
cargo build --target x86_64-unknown-linux-musl

# Improving load up performance

Loadup from GTFS week data = 194secs, 1 day = 178secs
Loadup from cache week data = 152secs, 1 day = 30secs

The cache is only a big advantage when it reduced the amount of data to read, so I could get a similar saving for development by prefiltering the GTFS data and then not having to support a separate cache format.

First I install the cargo flamegraph addon, I find flamegraphs are my favourite perf opimisation tool.

```
cargo install framegraph
```

On MacOS it needs to run as root:

The first run I tried it with loading from day cache, [flamegraph-1.svg] and 2 things stood out, the first was that deserialising Time takes 34% of the time, the other was that much of the time was blocked in read.

The second run I tried loading a day from the gtfs data, [flamegraph-2.svg], there writing the cache was 40%, so that's a big saving if we don't need the cache, departure_lookup which reads stop_times.txt was 56%. The vast majority of the time was in reading stop times, a large part of that again in parsing the Time (15% of total), and most of it was on parsing records that would be skipped. 

The third run was loading a week from the GTFS data with cache writing disabled, [flamegraph-3.svg]. 31% was in Time::deserialize, I expect to take that down to 5%. 15% was in MapAccess::next_key, which seems unnecessary 

[flamegraph-4.svg] just avoids creating an owned string when parsing time, Time::parse is still taking 25%.

[flamegraph-5.svg] removed the splitting in the time deserialisation, so no allocation is needed.

After improving the time parsing code:
Loadup from GTFS week data = 158s, 1 day = 110s
Loadup from cache week data = , 1 day = 

[flamegraph-7.svg] includes running a query for a while
