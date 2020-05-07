use futures::future;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use warp::{reject, Filter};

pub fn with_session<S: Sync + Send + From<u64>>(
) -> impl Filter<Extract = ((String, Arc<Mutex<S>>),), Error = reject::Rejection> + Clone {
    let container = Arc::new(SessionContainer::new());
    warp::query::<SessionKey>()
        .and_then(move |header| future::ready(container.session_filter(header)))
}

struct SessionContainer<S> {
    map: Mutex<HashMap<u64, Arc<Mutex<S>>>>,
    next_session_id: AtomicU64,
}

#[derive(serde::Deserialize)]
struct SessionKey {
    id: Option<u64>,
    count: Option<u32>,
}

#[derive(Debug)]
struct SessionOutOfSync;

impl reject::Reject for SessionOutOfSync {}

impl<S: From<u64>> SessionContainer<S> {
    fn new() -> SessionContainer<S> {
        SessionContainer {
            map: Mutex::new(HashMap::new()),
            next_session_id: AtomicU64::new(1000),
        }
    }

    pub fn session_filter(
        &self,
        key: SessionKey,
    ) -> Result<(String, Arc<Mutex<S>>), reject::Rejection> {
        let mut map = self.map.lock().unwrap();
        let session_id = key.id.unwrap_or_else(|| self.new_session_id());
        let update_number = key.count.unwrap_or(0);
        let session = map
            .entry(session_id)
            .or_insert_with(|| Arc::new(Mutex::new(From::from(session_id))));
        // if (*session.lock().unwrap()).update_number == update_number {
        Ok((session_id.to_string(), session.clone()))
        // } else {
        //     Err(reject::custom(SessionOutOfSync))
        // }
    }

    fn new_session_id(&self) -> u64 {
        self.next_session_id.fetch_add(1, Ordering::SeqCst)
    }
}
