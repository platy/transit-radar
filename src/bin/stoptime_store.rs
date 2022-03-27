use std::{path::Path, thread};

use transit_radar::gtfs::{self, db::GTFSSource, StopId, Time, TripId};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stop_times = read_stop_times("./gtfs")?;
    Ok(())
}

#[repr(C)]
#[derive(Debug)]
pub struct StopTime {
    // "trip_id","arrival_time","departure_time","stop_id","stop_sequence","pickup_type","drop_off_type","stop_headsign"
    /// Identifies a trip.
    pub trip_id: TripId,
    /// Arrival time at a specific stop for a specific trip on a route. If there are not separate times for arrival and departure at a stop, enter the same value for arrival_time and departure_time. For times occurring after midnight on the service day, enter the time as a value greater than 24:00:00 in HH:MM:SS local time for the day on which the trip schedule begins.
    /// Scheduled stops where the vehicle strictly adheres to the specified arrival and departure times are timepoints. If this stop is not a timepoint, it is recommended to provide an estimated or interpolated time. If this is not available, arrival_time can be left empty. Further, indicate that interpolated times are provided with timepoint=0. If interpolated times are indicated with timepoint=0, then time points must be indicated with timepoint=1. Provide arrival times for all stops that are time points. An arrival time must be specified for the first and the last stop in a trip.
    pub arrival_time: Time,
    /// Departure time from a specific stop for a specific trip on a route. For times occurring after midnight on the service day, enter the time as a value greater than 24:00:00 in HH:MM:SS local time for the day on which the trip schedule begins. If there are not separate times for arrival and departure at a stop, enter the same value for arrival_time and departure_time. See the arrival_time description for more details about using timepoints correctly.
    /// The departure_time field should specify time values whenever possible, including non-binding estimated or interpolated times between timepoints.
    pub departure_time: Time,
    // /// Identifies the serviced stop. All stops serviced during a trip must have a record in stop_times.txt. Referenced locations must be stops, not stations or station entrances. A stop may be serviced multiple times in the same trip, and multiple trips and routes may service the same stop.
    pub stop_id: StopId,
    // pub previous_stop_id: Option<StopId>,
    // pub next_stop_id: Option<StopId>,
    // /// Order of stops for a particular trip. The values must increase along the trip but do not need to be consecutive.
    // pub stop_sequence: u8,
    // pickup_type: u16,
    // drop_off_type: u16,
    // stop_headsign: Option<String>,
}

fn read_stop_times(
    gtfs_dir: impl AsRef<Path>,
) -> Result<Vec<StopTime>, Box<dyn std::error::Error>> {
    let mut stop_times = Vec::with_capacity(6_000_000);

    let source = &GTFSSource::new(gtfs_dir);

    let mut count_stop_id_invalid_digit = 0;
    let mut rdr = source.open_csv("stop_times.txt")?;
    for result in rdr.deserialize::<gtfs::StopTime>() {
        match result {
            Ok(gtfs::StopTime {
                trip_id,
                arrival_time,
                departure_time,
                stop_id,
                stop_sequence: _,
            }) => {
                let stop_time = StopTime {
                    trip_id,
                    arrival_time,
                    departure_time,
                    stop_id,
                };
                stop_times.push(stop_time);
            }
            Err(err) => {
                if let csv::ErrorKind::Deserialize { pos: _, err } = err.kind() {
                    if err.field() == Some(3) {
                        if let csv::DeserializeErrorKind::ParseInt(err) = err.kind() {
                            if std::num::IntErrorKind::InvalidDigit == *err.kind() {
                                count_stop_id_invalid_digit += 1;
                                continue;
                            }
                        }
                    }
                }
                eprintln!("Error parsing stop time : {}", err)
            }
        }
    }
    eprintln!(
        "{} stop times failed to parse due to an invalid digit in the stop id, this happens",
        count_stop_id_invalid_digit
    );
    Ok(stop_times)
}
