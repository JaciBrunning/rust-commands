use std::{sync::Arc, pin::Pin};

use futures::Future;
pub use rust_commands_macros::Systems;
use tokio::sync::{RwLock, oneshot, Mutex};

#[macro_export]
macro_rules! pinbox {
  ($fnpath:expr) => {
    |x| async move { $fnpath(x).await }.boxed()
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Priority(pub usize);

pub trait Systems {
  type Shared;

  fn shared(self) -> Self::Shared;
}

pub enum MaybeReferred<T> {
  Owned(T),
  Referred(Priority, oneshot::Sender<oneshot::Sender<T>>)
}

impl<T> MaybeReferred<T> {
  pub fn can_take(&mut self, priority: Priority) -> bool {
    match self {
      MaybeReferred::Owned(_) => true,
      MaybeReferred::Referred(p, _) if *p < priority => true,
      _ => false
    }
  }

  pub async fn try_take(&mut self, priority: Priority, take_back: oneshot::Sender<oneshot::Sender<T>>) -> Option<T> {
    if self.can_take(priority) {
      let old = std::mem::replace(self, MaybeReferred::Referred(priority, take_back));
      match old {
        MaybeReferred::Owned(o) => Some(o),
        MaybeReferred::Referred(_, taker) => {
          // Take it from another task
          let (tx, rx) = oneshot::channel();
          taker.send(tx).ok();
          Some(rx.await.unwrap())
        },
      }
    } else {
      None
    }
  }
}

pub struct System<T> {
  storage: Mutex<MaybeReferred<T>>
}

impl<T> System<T> {
  pub fn new(system: T) -> Self {
    Self { storage: Mutex::new(MaybeReferred::Owned(system)) }
  }

  // pub async fn perform<O, Fut: Future<Output = O>, F: FnOnce(&mut T) -> Fut>(&self, priority: Priority, f: F) -> Option<O> {
  pub async fn perform<O, F>(self: Arc<Self>, priority: Priority, f: F) -> Option<O>
    where F: FnOnce(&mut T) -> Pin<Box<dyn Future<Output = O> + '_ + Send>>
  {
    let (tx_take, rx_take) = oneshot::channel();

    let val = {
      self.storage.lock().await.try_take(priority, tx_take).await
    };

    if let Some(mut sys) = val {
      let future = f(&mut sys);

      tokio::select! {
        ret = future => {
          // Reset the storage to be owned
          *self.storage.lock().await = MaybeReferred::Owned(sys);
          Some(ret)
        },
        new_sender = rx_take => {
          new_sender.unwrap().send(sys).ok();
          None
        }
      }
    } else {
      None
    }
  }
}

#[macro_export]
macro_rules! perform {
  ($system:expr, $priority:expr, $func:expr) => {
    tokio::task::spawn($system.clone().perform($priority, $func))
  }
}

// This would all be taken care of with a macro for an arbitrary size of tuple

#[async_trait::async_trait]
pub trait TuplePerform02 {
  type T1;
  type T2;

  async fn perform<O: Send, F>(self, priority: Priority, f: F) -> Option<O>
    where F: for<'a> FnOnce((&'a mut Self::T1, &'a mut Self::T2)) -> Pin<Box<dyn Future<Output = O> + 'a + Send>> + Send;
}

#[async_trait::async_trait]
impl<T1: Send, T2: Send> TuplePerform02 for (Arc<System<T1>>, Arc<System<T2>>) {
  type T1 = T1;
  type T2 = T2;

  async fn perform<O: Send, F>(self, priority: Priority, f: F) -> Option<O>
    where F: for<'a> FnOnce((&'a mut Self::T1, &'a mut Self::T2)) -> Pin<Box<dyn Future<Output = O> + 'a + Send>> + Send
  {
    let (take1_tx, take1_rx) = oneshot::channel();
    let (take2_tx, take2_rx) = oneshot::channel();

    let mut vals = {
      let mut lock1 = self.0.storage.lock().await;
      let mut lock2 = self.1.storage.lock().await;

      if lock1.can_take(priority) && lock2.can_take(priority) {
        ( lock1.try_take(priority, take1_tx).await.unwrap(), lock2.try_take(priority, take2_tx).await.unwrap() )
      } else {
        return None;
      }
    };

    let future = f((&mut vals.0, &mut vals.1));

    tokio::select! {
      ret = future => {
        // Reset the storage to be owned
        *self.0.storage.lock().await = MaybeReferred::Owned(vals.0);
        *self.1.storage.lock().await = MaybeReferred::Owned(vals.1);
        Some(ret)
      },
      // Have to make sure whatever gets triggered is last to be released, since the others have to be put into Owned
      // before they can be taken out.
      new_sender = take1_rx => {
        *self.1.storage.lock().await = MaybeReferred::Owned(vals.1);
        new_sender.unwrap().send(vals.0).ok();
        None
      },
      new_sender = take2_rx => {
        *self.0.storage.lock().await = MaybeReferred::Owned(vals.0);
        new_sender.unwrap().send(vals.1).ok();
        None
      }
    }
  }
}