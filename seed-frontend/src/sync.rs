//! Data fetching module which fetches required data based on requirements from the client, the server can send increments of the data required to meet the requirements.

use futures::prelude::*;
use radar_search::naive_sync::SyncData;
use seed::{error, fetch, prelude::*};
use serde::{de::DeserializeOwned, Serialize};

pub enum Msg<D, I> {
    DataFetched(Result<SyncData<D, I>, LoadError>),
    FetchData,
}

pub struct Model<D> {
    status: RequestStatus,
    sync: State<D>,
}

impl<D> Default for Model<D> {
    fn default() -> Model<D> {
        Model {
            status: RequestStatus::Ready,
            sync: State::NotSynced,
        }
    }
}

#[derive(Eq, PartialEq)]
enum RequestStatus {
    /// no request is being made
    Ready,
    /// a request is being made, parameter is the timestamp that it was made at
    InProgress, //(u64),
    /// a request is being made and another request is needed, parameter is the timestamp that it was made at
    Invalidated, //(u64),
}

// impl RequestStatus {
//     fn request_allowed(&self) -> bool {
//         match self {
//             Self::Ready => true,
//             Self::InProgress(request_made) =>
//         }
//     }
// }

enum State<D> {
    NotSynced,
    Synced {
        session_id: u64,
        update_count: u64,
        data: D,
    },
}

pub fn update<D: 'static, I: 'static>(
    msg: Msg<D, I>,
    model: &mut Model<D>,
    url: String,
    orders: &mut impl Orders<Msg<D, I>>,
) -> bool
where
    D: std::ops::AddAssign<I> + DeserializeOwned,
    I: DeserializeOwned,
{
    match msg {
        Msg::FetchData => {
            match model.status {
                RequestStatus::Ready => {
                    orders.perform_cmd(request(model.url(url)).map(Msg::<D, I>::DataFetched));
                    orders.skip();
                    model.status = RequestStatus::InProgress;
                }
                _ => {
                    model.status = RequestStatus::Invalidated;
                }
            }
            false
        }

        Msg::DataFetched(Ok(data)) => {
            model.receive(data);
            match model.status {
                RequestStatus::Ready => {
                    panic!("unexpected response data");
                }
                RequestStatus::InProgress => {
                    model.status = RequestStatus::Ready;
                }
                RequestStatus::Invalidated => {
                    model.status = RequestStatus::Ready;
                    orders.send_msg(Msg::FetchData);
                }
            }
            true
        }

        Msg::DataFetched(Err(fail_reason)) => {
            error!(format!(
                "Fetch error - Fetching repository info failed - {:#?}",
                fail_reason
            ));
            model.status = RequestStatus::Ready;
            orders.skip();
            false
        }
    }
}

async fn request<S>(url: String) -> Result<S, LoadError>
where
    S: DeserializeOwned,
{
    let response = fetch::fetch(url).await?;
    let body = response.bytes().await?;
    Ok(rmp_serde::from_read_ref(&body)?)
}

impl<D> Model<D> {
    /// todo use a header instead and leave the url to the caller
    pub fn url(&self, mut url: String) -> String {
        if let State::Synced {
            session_id,
            update_count,
            data: _,
        } = self.sync
        {
            let query = serde_urlencoded::to_string(SyncParams {
                id: session_id,
                count: update_count,
            })
            .unwrap();
            url += "&";
            url += &query;
        }
        url
    }

    // todo check update numbers
    pub fn receive<'de, I>(&mut self, sync_data: SyncData<D, I>) -> &D
    where
        D: std::ops::AddAssign<I>,
    {
        match sync_data {
            SyncData::Initial {
                session_id,
                update_number: update_count,
                data,
            } => {
                self.sync = State::Synced {
                    session_id,
                    update_count,
                    data,
                };
                self.get().unwrap()
            }

            SyncData::Increment {
                increment,
                update_number,
                session_id,
            } => {
                if let State::Synced {
                    session_id: our_session_id,
                    update_count,
                    data: existing_data,
                } = &mut self.sync
                {
                    *existing_data += increment;
                    *update_count = update_number;
                    assert!(session_id == *our_session_id, "session ids don't match");
                    &*existing_data
                } else {
                    panic!("bad sync: retrieved increment with no data locally");
                }
            }
        }
    }

    pub fn get(&self) -> Option<&D> {
        match &self.sync {
            State::NotSynced => None,
            State::Synced {
                data,
                update_count: _,
                session_id: _,
            } => Some(data),
        }
    }

    pub fn never_requested(&self) -> bool {
        self.status == RequestStatus::Ready && self.get().is_none()
    }
}

#[derive(Serialize)]
struct SyncParams {
    id: u64,
    count: u64,
}

#[derive(Debug)]
pub enum LoadError {
    FetchError(fetch::FetchError),
    RMPError(rmp_serde::decode::Error),
}

impl From<fetch::FetchError> for LoadError {
    fn from(error: fetch::FetchError) -> LoadError {
        Self::FetchError(error)
    }
}

impl From<rmp_serde::decode::Error> for LoadError {
    fn from(error: rmp_serde::decode::Error) -> LoadError {
        Self::RMPError(error)
    }
}
