use gloo_timers::callback::Timeout;
use js_sys::Date;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::mem;
use std::rc::Rc;

#[derive(Clone)]
pub struct Scheduler(Rc<RefCell<SchedulerState>>);

enum SchedulerState {
    Empty,
    // these seem to be false positives with the `dead_code check
    #[allow(dead_code)]
    Scheduled {
        next_timeout: Timeout,
        scheduled_wakes: BTreeMap<u64, Box<dyn FnOnce() -> ()>>,
    },
}

impl SchedulerState {
    fn schedule<F>(&mut self, millis_to: u32, timestamp: u64, f: F, scheduler: &Scheduler)
    where
        F: 'static + FnOnce() -> (),
    {
        match self {
            Self::Empty => {
                let mut scheduled_wakes: BTreeMap<u64, Box<dyn FnOnce() -> ()>> = BTreeMap::new();
                scheduled_wakes.insert(timestamp, Box::new(f));
                *self = Self::Scheduled {
                    next_timeout: Timeout::new(millis_to as u32, scheduler.waker()),
                    scheduled_wakes,
                };
            }
            Self::Scheduled {
                ref mut next_timeout,
                ref mut scheduled_wakes,
            } => {
                if timestamp < *scheduled_wakes.iter().next().unwrap().0 {
                    *next_timeout = Timeout::new(millis_to as u32, scheduler.waker());
                }
                scheduled_wakes.insert(timestamp, Box::new(f));
            }
        }
    }

    fn remove_elapsed(&mut self, scheduler: &Scheduler) -> Vec<Box<dyn FnOnce()>> {
        match self {
            Self::Empty => panic!("unexpected wake on empty scheduler"),
            Self::Scheduled {
                ref mut next_timeout,
                ref mut scheduled_wakes,
            } => {
                let time = Date::now() as u64;
                let not_elapsed = scheduled_wakes.split_off(&time);
                let elapsed = mem::replace(scheduled_wakes, not_elapsed)
                    .into_iter()
                    .map(|(_k, v)| v)
                    .collect();

                if let Some((first_schedule, _)) = scheduled_wakes.iter().next() {
                    let millis_to = first_schedule - time;
                    *next_timeout = Timeout::new(millis_to as u32, scheduler.waker());
                } else {
                    *self = Self::Empty;
                }

                elapsed
            }
        }
    }
}

impl Scheduler {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(SchedulerState::Empty)))
    }

    /// Schdules a function to be called or runs immediately if for a time in the past
    pub fn schedule<F>(&self, timestamp: u64, f: F)
    where
        F: 'static + FnOnce() -> (),
    {
        let millis_to = timestamp as i64 - (Date::now() as i64);
        if millis_to < 0 {
            self.0.borrow_mut().schedule(0, timestamp, f, self);
        } else {
            self.0
                .borrow_mut()
                .schedule(millis_to as u32, timestamp, f, self);
        }
    }

    fn waker(&self) -> impl FnOnce() {
        let s = self.clone();
        move || {
            s.wake();
        }
    }

    fn wake(&self) {
        let elapsed = self.0.borrow_mut().remove_elapsed(self);
        for f in elapsed {
            f();
        }
    }
}
