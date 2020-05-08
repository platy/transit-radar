use futures::future;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use warp::{reject, Filter};
use radar_search::search_data_sync::ClientSession;

pub fn with_session<S: Sync + Send + ClientSession>() -> impl Filter<Extract = (Arc<Mutex<S>>,), Error = reject::Rejection> + Clone {
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
    count: Option<u64>,
}

impl<S: ClientSession> SessionContainer<S> {
    fn new() -> SessionContainer<S> {
        SessionContainer {
            map: Mutex::new(HashMap::new()),
            next_session_id: AtomicU64::new(1000),
        }
    }

    pub fn session_filter(
        &self,
        key: SessionKey,
    ) -> Result<Arc<Mutex<S>>, reject::Rejection> {
        let mut map = self.map.lock().unwrap();
        let session_id = key.id.unwrap_or_else(|| self.new_session_id());
        let update_number = key.count.unwrap_or(0);
        let session = map
            .entry(session_id)
            .or_insert_with(|| Arc::new(Mutex::new(S::new(session_id))));
        let server_update_number = (*session.lock().unwrap()).update_number();
        if server_update_number != update_number {
            eprintln!("session {} out of sync, client {}, server {} - resetting", session_id, update_number, server_update_number);
            *session = Arc::new(Mutex::new(S::new(session_id)));
        }
        Ok(session.clone())
    }

    fn new_session_id(&self) -> u64 {
        self.next_session_id.fetch_add(1, Ordering::SeqCst)
    }
}
