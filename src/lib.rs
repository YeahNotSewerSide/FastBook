use parking_lot::RwLock;
use std::collections::BTreeMap;
use std::sync::atomic::{fence, AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Clone)]
pub struct OrderBookRead<T>
where
    T: Ord + Eq,
{
    bids: *mut BTreeMap<T, AtomicU64>,
    asks: *mut BTreeMap<T, AtomicU64>,
    lock_bids: *mut RwLock<bool>,
    lock_asks: *mut RwLock<bool>,
    allocated: Arc<AtomicBool>,
}

pub struct OrderBookWrite<T: Ord + Eq> {
    bids: *mut BTreeMap<T, AtomicU64>,
    asks: *mut BTreeMap<T, AtomicU64>,
    lock_bids: *mut RwLock<bool>,
    lock_asks: *mut RwLock<bool>,
    allocated: Arc<AtomicBool>,
}

pub fn orderbook<T: Ord + Eq>() -> (OrderBookWrite<T>, OrderBookRead<T>) {
    let bids = Box::into_raw(Box::default());
    let asks = Box::into_raw(Box::default());
    let lock_bids = Box::into_raw(Box::new(RwLock::new(false)));
    let lock_asks = Box::into_raw(Box::new(RwLock::new(false)));
    let allocated = Arc::new(AtomicBool::new(true));

    (
        OrderBookWrite {
            bids,
            asks,
            lock_bids,
            lock_asks,
            allocated: allocated.clone(),
        },
        OrderBookRead {
            bids,
            asks,
            lock_bids,
            lock_asks,
            allocated,
        },
    )
}

unsafe impl<T: Send + Ord + Eq> Sync for OrderBookRead<T> {}
unsafe impl<T: Send + Ord + Eq> Send for OrderBookRead<T> {}

unsafe impl<T: Send + Ord + Eq> Send for OrderBookWrite<T> {}

impl<T: Ord + Eq + Copy> OrderBookWrite<T> {
    pub fn update_bids(&mut self, bids: &[(T, f64)]) {
        let inner_bids = unsafe { &mut *self.bids };
        for bid in bids {
            if bid.1 == 0.0 {
                let guard = unsafe { &mut *self.lock_bids }.write();
                fence(Ordering::Acquire);
                inner_bids.remove(&bid.0);
                fence(Ordering::Release);

                drop(guard);
            } else {
                let entry = inner_bids.get(&bid.0);
                match entry {
                    Some(val) => {
                        val.swap(bid.1.to_bits(), Ordering::Relaxed);
                    }
                    None => {
                        let guard = unsafe { &mut *self.lock_bids }.write();
                        fence(Ordering::Acquire);

                        inner_bids.insert(bid.0, AtomicU64::new(bid.1.to_bits()));
                        fence(Ordering::Release);

                        drop(guard);
                    }
                }
            }
        }
    }

    pub fn update_asks(&mut self, asks: &[(T, f64)]) {
        let inner_asks = unsafe { &mut *self.asks };
        for ask in asks {
            if ask.1 == 0.0 {
                let guard = unsafe { &mut *self.lock_asks }.write();
                fence(Ordering::Acquire);

                inner_asks.remove(&ask.0);
                fence(Ordering::Release);

                drop(guard);
            } else {
                let entry = inner_asks.get(&ask.0);
                match entry {
                    Some(val) => {
                        val.swap(ask.1.to_bits(), Ordering::Relaxed);
                    }
                    None => {
                        let guard = unsafe { &mut *self.lock_asks }.write();
                        fence(Ordering::Acquire);

                        inner_asks.insert(ask.0, AtomicU64::new(ask.1.to_bits()));
                        fence(Ordering::Release);

                        drop(guard);
                    }
                }
            }
        }
    }
}

impl<T: Ord + Eq + Copy> OrderBookRead<T> {
    pub fn best_bid(&self) -> Result<Option<(T, f64)>, &'static str> {
        if !self.allocated.load(Ordering::Acquire) {
            return Err("deallocated");
        }
        let guard = unsafe { &*self.lock_bids }.read();

        fence(Ordering::Acquire);
        // we do not change the underlying entry
        let bids = unsafe { &mut *self.bids };
        let entry = match bids.last_entry() {
            Some(e) => e,
            None => return Ok(None),
        };

        let key = *entry.key();
        let value = f64::from_bits(entry.get().load(Ordering::Relaxed));

        fence(Ordering::Release);
        drop(guard);

        Ok(Some((key, value)))
    }

    pub fn best_ask(&self) -> Result<Option<(T, f64)>, &'static str> {
        if !self.allocated.load(Ordering::Acquire) {
            return Err("deallocated");
        }
        let guard = unsafe { &*self.lock_asks }.read();

        fence(Ordering::Acquire);
        // we do not change the underlying entry
        let asks = unsafe { &mut *self.asks };
        let entry = match asks.first_entry() {
            Some(e) => e,
            None => return Ok(None),
        };

        let key = *entry.key();
        let value = f64::from_bits(entry.get().load(Ordering::Relaxed));

        fence(Ordering::Release);
        drop(guard);

        Ok(Some((key, value)))
    }
}

impl<T: Eq + Ord> Drop for OrderBookWrite<T> {
    fn drop(&mut self) {
        self.allocated.swap(false, Ordering::Acquire);
        let guard_bids = unsafe { &*self.lock_bids }.write();
        let guard_asks = unsafe { &*self.lock_asks }.write();
        fence(Ordering::Acquire);
        let _ = Box::leak(unsafe { Box::from_raw(self.bids) });
        let _ = Box::leak(unsafe { Box::from_raw(self.asks) });
        let _ = Box::leak(unsafe { Box::from_raw(self.lock_bids) });
        let _ = Box::leak(unsafe { Box::from_raw(self.lock_asks) });
        fence(Ordering::Release);
        drop(guard_bids);
        drop(guard_asks);
    }
}
