//! A task manager for parallel queues that process tasks serially.
//!
//! When a new queue is created the task manager will spawn a worker
//! thread that immediately blocks. When `add_task()` is called with
//! the key for that queue the manager will add the task to the queue
//! and fire a signal to the worker thread for that queue that there's
//! a new task waiting. The worker thread then pops a task off the
//! queue and calls its `run` method. Once there are no more tasks in
//! the queue the worker thread will go back to sleep.
//!
//! # Limitations
//!
//! - Task queues can grow infinitely. We should have optional a way
//! to cap the queue size and any tasks that come in after that limit
//! push off the oldest task.
//!
//! - There is no way to signal a shutdown to worker threads.
//!
//! # Examples
//!
//! ## Waiting for tasks to finish
//!
//! ```
//! use deployer::task_manager::{TaskManager, Runnable};
//! use std::thread;
//!
//! struct Task { msg: &'static str, delay: u32 };
//! impl Runnable for Task {
//!     fn run(&mut self) {
//!         thread::sleep_ms(self.delay);
//!         println!("{}", self.msg);
//!     }
//! }
//!
//! let mut task_manager = TaskManager::new();
//!
//! // This will cause "a", "b", "c" and "1", "2", "3" to print in
//! // order though letters and numbers will be intermingled because the
//! // "letters" and "numbers" queues process in parallel.
//! let last_letter_task = {
//!     task_manager.ensure_queue("letters");
//!     task_manager.add_task("letters", Task {msg: "a", delay: 500});
//!     task_manager.add_task("letters", Task {msg: "b", delay: 100});
//!     task_manager.add_task("letters", Task {msg: "c", delay: 200})
//! };
//!
//! let last_number_task = {
//!     task_manager.ensure_queue("numbers");
//!     task_manager.add_task("letters", Task {msg: "1", delay: 200});
//!     task_manager.add_task("letters", Task {msg: "2", delay: 100});
//!     task_manager.add_task("letters", Task {msg: "3", delay: 500})
//! };
//!
//! last_number_task.unwrap().recv();
//! last_letter_task.unwrap().recv();
//! ```
//!
//! ## Getting results of a task
//!
//! ```
//! use deployer::task_manager::{TaskManager, Runnable};
//!
//! # fn do_some_hard_work() { }
//! struct LongRunningTask {
//!     result: Option<u64>,
//! }
//! impl LongRunningTask {
//!     fn new() -> LongRunningTask {
//!       LongRunningTask { result: None }
//!     }
//! }
//! impl Runnable for LongRunningTask {
//!     fn run(&mut self) {
//!         do_some_hard_work();
//!         self.result = Some(42);
//!     }
//! }
//!
//! let mut task_manager = TaskManager::new();
//!
//! task_manager.ensure_queue("q");
//!
//! let task_rx = task_manager.add_task("q", LongRunningTask::new()).unwrap();
//! let task = task_rx.recv().unwrap();
//! assert_eq!(task.result, Some(42));
//! ```
//!
//! ## Blocking indefinitely
//!
//! ```
//! use std::sync::{Arc, Mutex};
//! use deployer::task_manager::{TaskManager, Runnable};
//! use std::thread;
//!
//! # fn do_some_work() { }
//! struct Task;
//! impl Runnable for Task {
//!     fn run(&mut self) { do_some_work() }
//! }
//!
//! let task_manager = Arc::new(Mutex::new(TaskManager::new()));
//! {
//!     let shared_manager = task_manager.clone();
//!     thread::spawn(move || {
//!         let locked_manager = shared_manager.lock().unwrap();
//!         task_manager.ensure_queue("q");
//!         task_manager.add_task("q", Task {});
//!     })
//! }
//!
//! {
//!     let shared_manager = task_manager.clone();
//!     thread::spawn(move || {
//!         let locked_manager = shared_manager.lock().unwrap();
//!         task_manager.shutdown_gracefully();
//!     })
//! }
//!
//! task_manager.wait();
//! println!("task manager done");
//! ```

use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::fmt;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::thread;

/// Types that are able to be added to a [TaskManager](./index.html) queue.
pub trait Runnable {
    fn run(&mut self);
}

struct Queue<T> where T: Runnable + Send {
    queue: VecDeque<(T, Sender<T>)>,
}
impl<T> Queue<T> where T: Runnable + Send {
    fn new() -> Queue<T> { Queue { queue: VecDeque::new() } }
    fn push_task(&mut self, task: (T, Sender<T>)) {
        self.queue.push_back(task);
    }
    fn pop_task(&mut self) -> Option<(T, Sender<T>)> {
        self.queue.pop_front()
    }
}

type QueueMap<'a, T> = BTreeMap<&'a str, Arc<Mutex<Queue<T>>>>;
type ThreadMap<'a> = BTreeMap<&'a str, (JoinHandle<()>, Sender<()>)>;

pub struct TaskManager<'a, T> where T: 'static + Runnable + Send {
    queues: QueueMap<'a, T>,
    threads: ThreadMap<'a>,
    shutdown_lock: (Sender<()>, Receiver<()>)
}

#[derive(Debug)]
pub enum Error {
    QueueMissing,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match *self {
            Error::QueueMissing => "could not find queue in queue map",
        })
    }
}

impl<'a, T> TaskManager<'a, T> where T: 'static + Runnable + Send {
    /// Create a new TaskManager
    pub fn new() -> TaskManager<'a, T> {
        TaskManager {
            queues: QueueMap::<T>::new(),
            threads: ThreadMap::new(),
            shutdown_lock: channel(),
        }
    }

    /// Add a task to a queue. When the task is complete it will be sent over
    /// the returned `Receiver`.
    ///
    /// # Failures
    ///
    /// - `QueueMissing`: could not find the queue. This will only happen if
    /// an `add_task()` call happens before an `ensure_queue()` call for
    /// that queue.
    pub fn add_task(&mut self, queue_key: &'a str, task: T) -> Result<Receiver<T>, Error> {
        let (task_tx, task_rx) = channel();
        {
            let mut locked_queue = match self.queues.get_mut(queue_key) {
                // Safe unwrap: With the current implementation it's
                // impossible for a lock to get poisoned. There is exactly
                // one other spot where we acquire a lock: in the worker
                // thread and the only thing we do is `pop_task`, an alias
                // for `pop_front` on the underlying VecDeque, which cannot
                // cause a thread panic. In this method we only do one thing
                // while holding the lock, `push_task` (an alias for
                // `push_back`) which also cannot cause a thread panic.
                Some(queue_mutex) => queue_mutex.lock().unwrap(),
                None => return Err(Error::QueueMissing),
            };
            locked_queue.push_task((task, task_tx));
        }

        // Safe unwrap: If the queue exists, a corresponding thread in the
        // thread map is guaranteed to exist because both maps are private
        // and we always create a thread map entry when we create a queue
        // map entry.
        let worker_tx = self.get_channel(queue_key).unwrap();

        // Safe unwrap: The worker thread doesn't perform any operations
        // that can cause a panic.
        worker_tx.send(()).unwrap();

        Ok(task_rx)
    }

    pub fn wait(&self) {
        let (_, ref rx) = self.shutdown_lock;
        rx.recv();
    }

    pub fn shutdown(&self) {

    }

    /// Create a queue only if one doesn't already exist with that key
    pub fn ensure_queue(&mut self, queue_key: &'a str) {
        if self.queues.contains_key(queue_key) { return }

        let queue = Arc::new(Mutex::new(Queue::<T>::new()));
        self.queues.insert(queue_key, queue);

        // Safe unwrap: We just inserted it above.
        self.start_worker(queue_key);
    }

    fn find(&mut self, key: &'a str) -> Option<&mut Arc<Mutex<Queue<T>>>> {
        self.queues.get_mut(key)
    }

    fn get_channel(&self, key: &'a str) -> Option<&Sender<()>> {
        match self.threads.get(key) {
            Some(&(_, ref tx)) => Some(tx),
            None => None,
        }
    }

    // TODO: remove once we can use the `result` from the task run.
    #[allow(unused_must_use)]
    fn start_worker(&mut self, key: &'a str) {
        let queue = self.find(key).unwrap().clone();
        let (worker_tx, worker_rx) = channel();
        let worker = thread::spawn(move || {
            loop {
                if worker_rx.recv().is_err() {
                    // This will only happen if the manager gets
                    // deallocated, which will typically happen if the main
                    // thread is in the process of shutting down.
                    break;
                }

                // Safe unwrap: Impossible for lock to get poisoned, see
                // comment in `add_task()`.
                let possible_task = queue.lock().unwrap().pop_task();

                if let Some((mut task, task_tx)) = possible_task {
                    // Protect the worker thread from any panics that would
                    // be caused by `task.run()`.
                    thread::spawn(move || {
                        task.run();
                        task_tx.send(task);
                    }).join();
                }
            };
        });
        self.threads.insert(key, (worker, worker_tx));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::sync::{Arc, Mutex};

    struct Task {
        s: Arc<Mutex<String>>,
        m: &'static str,
    }

    impl Runnable for Task {
        fn run(&mut self) {
            let mut s = self.s.lock().unwrap();
            s.push_str(self.m);
        }
    }

    #[test]
    #[allow(unused_must_use)]
    fn test_task_manager() {
        let s1 = Arc::new(Mutex::new(String::new()));
        let s2 = Arc::new(Mutex::new(String::new()));

        let task_manager = Arc::new(Mutex::new(TaskManager::new()));
        let thread1 = {
            let shared_manager = task_manager.clone();
            let s = s1.clone();
            thread::spawn(move || {
                let mut manager = shared_manager.lock().unwrap();
                manager.ensure_queue("a");
                manager.add_task("a", Task {s: s.clone(), m: "b"});
                manager.add_task("a", Task {s: s.clone(), m: "r"});
                manager.add_task("a", Task {s: s.clone(), m: "i"});
                manager.add_task("a", Task {s: s.clone(), m: "a"});
                manager.add_task("a", Task {s: s.clone(), m: "n"})
                    .unwrap().recv();

            })
        };
        let thread2 = {
            let shared_manager = task_manager.clone();
            let s = s2.clone();
            thread::spawn(move || {
                let mut manager = shared_manager.lock().unwrap();
                manager.ensure_queue("b");
                manager.add_task("b", Task {s: s.clone(), m: "s"});
                manager.add_task("b", Task {s: s.clone(), m: "l"});
                manager.add_task("b", Task {s: s.clone(), m: "o"});
                manager.add_task("b", Task {s: s.clone(), m: "t"});
                manager.add_task("b", Task {s: s.clone(), m: "h"});
                manager.add_task("b", Task {s: s.clone(), m: "s"})
                    .unwrap().recv();
            })
        };

        thread1.join(); thread2.join();

        assert_eq!(*s1.lock().unwrap(), "brian");
        assert_eq!(*s2.lock().unwrap(), "sloths");
    }
}
