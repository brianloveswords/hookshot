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
//! See docs for the [`TaskManager`](struct.TaskManager.html) struct for more
//! usage examples.
//!
//! # Examples
//!
//! ## Waiting for tasks to finish
//!
//! ```
//! use hookshot::task_manager::{TaskManager, Runnable};
//! use std::thread;
//!
//! struct Task { msg: &'static str, delay: u32 };
//! impl Runnable for Task {
//!     fn run(&mut self) {
//!         thread::sleep_ms(self.delay);
//!         println!("{}", self.msg);
//!     }
//! }
//! // Set a limit of 100 items per queue
//! let limit = Some(100);
//! let mut task_manager = TaskManager::new(limit);
//!
//! // This will cause "a", "b", "c" and "1", "2", "3" to print in
//! // order though letters and numbers will be intermingled because the
//! // "letters" and "numbers" queues process in parallel.
//! let last_letter_task = {
//!     let queue = task_manager.ensure_queue(String::from("letters"));
//!     task_manager.add_task(&queue, Task {msg: "a", delay: 500});
//!     task_manager.add_task(&queue, Task {msg: "b", delay: 100});
//!     task_manager.add_task(&queue, Task {msg: "c", delay: 200})
//! };
//!
//! let last_number_task = {
//!     let queue = task_manager.ensure_queue(String::from("numbers"));
//!     task_manager.add_task(&queue, Task {msg: "1", delay: 200});
//!     task_manager.add_task(&queue, Task {msg: "2", delay: 100});
//!     task_manager.add_task(&queue, Task {msg: "3", delay: 500})
//! };
//!
//! last_number_task.unwrap().recv();
//! last_letter_task.unwrap().recv();
//! ```
//!
//! ## Getting results of a task
//!
//! ```
//! use hookshot::task_manager::{TaskManager, Runnable};
//!
//! # fn do_some_hard_work() { }
//! struct LongRunningTask {
//!     result: Option<u64>,
//! }
//! impl LongRunningTask {
//!     fn new() -> LongRunningTask {
//!         LongRunningTask { result: None }
//!     }
//! }
//! impl Runnable for LongRunningTask {
//!     fn run(&mut self) {
//!         do_some_hard_work();
//!         self.result = Some(42);
//!     }
//! }
//! // Allow queues to grow without bound
//! let limit = None;
//! let mut task_manager = TaskManager::new(limit);
//!
//! let key = task_manager.ensure_queue(String::from("q"));
//!
//! let task_rx = task_manager.add_task(&key, LongRunningTask::new()).unwrap();
//! let task = task_rx.recv().unwrap();
//! assert_eq!(task.result, Some(42));
//! ```
//!
//! ## Graceful shutdowns
//! ```
//! # use hookshot::task_manager::{TaskManager, Runnable};
//! # use std::thread;
//! # use std::sync::{Arc, Mutex};
//! # use std::sync::mpsc::channel;
//! #
//! # fn event_handler<F>(name: &'static str, func: F)
//! #     where F: FnOnce()+ Send + 'static {
//! #         println!("adding event handler for {}", name);
//! #         thread::spawn(func);
//! #     }
//! #
//! # struct ImportantTask;
//! # impl Runnable for ImportantTask {
//! #     fn run(&mut self) { println!("task added") }
//! # }
//! # impl ImportantTask { fn new() -> ImportantTask { ImportantTask } }
//! let (shutdown_tx, shutdown_rx) = channel();
//! let task_manager = Arc::new(Mutex::new(TaskManager::new_with_lock(None, shutdown_tx)));
//!
//! let shared_manager = task_manager.clone();
//! event_handler("add_task", move || {
//!     let mut locked_manager = shared_manager.lock().unwrap();
//!     let queue = locked_manager.ensure_queue(String::from("q"));
//!     locked_manager.add_task(&queue, ImportantTask::new()).unwrap();
//! });
//!
//! let shared_manager = task_manager.clone();
//! event_handler("shutdown", move || {
//!     let mut locked_manager = shared_manager.lock().unwrap();
//!
//!     // `shutdown()` lets each worker finish a final task if it's already
//!     // working on one and once each worker is done it sends a message down
//!     // the lock channel if one exists.
//!     locked_manager.shutdown();
//! });
//!
//! // This blocks until a call to `shutdown()` is completed.
//! shutdown_rx.recv().unwrap();
//! println!("task manager done");

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
    fn cancel(&self) { }
}

struct Queue<T>
    where T: Runnable + Send
{
    queue: VecDeque<(T, Sender<T>)>,
    limit: Option<u64>,
}
impl<T> Queue<T> where T: Runnable + Send {
    fn new(limit: Option<u64>) -> Queue<T> {
        Queue { queue: VecDeque::new(), limit: limit }
    }
    fn push_task(&mut self, task: (T, Sender<T>)) {
        if let Some(limit) = self.limit {
            if limit < 1 {
                return;
            }
            if self.queue.len() + 1 > limit as usize {
                if let Some((cancelled_task, _)) = self.pop_task() {
                    cancelled_task.cancel();
                }
                return self.push_task(task);
            }
        }
        self.queue.push_back(task);
    }
    fn pop_task(&mut self) -> Option<(T, Sender<T>)> {
        self.queue.pop_front()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    QueueMissing,
    Shutdown,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match *self {
            Error::QueueMissing => "could not find queue in queue map",
            Error::Shutdown => "manager is shut down",
        })
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub struct QueueKey {
    k: String,
}

type QueueMap<T> = BTreeMap<QueueKey, Arc<Mutex<Queue<T>>>>;
type ThreadMap = BTreeMap<QueueKey, (JoinHandle<()>, Sender<()>)>;

pub struct TaskManager<T>
    where T: 'static + Runnable + Send
{
    queues: QueueMap<T>,
    threads: ThreadMap,
    shutdown_lock: Option<Sender<()>>,
    stopped: bool,
    limit: Option<u64>,
}

impl<'a, T> TaskManager<T> where T: 'static + Runnable + Send {
    /// Create a new TaskManager
    pub fn new(limit: Option<u64>) -> TaskManager<T> {
        TaskManager {
            queues: QueueMap::<T>::new(),
            threads: ThreadMap::new(),
            shutdown_lock: None,
            stopped: false,
            limit: limit,
        }
    }

    /// Create a new TaskManager that takes a shutdown receiver which can be
    /// used to block a thread until [`shutdown()`](#method.shutdown) is called.
    pub fn new_with_lock(limit: Option<u64>, lock: Sender<()>) -> TaskManager<T> {
        TaskManager {
            queues: QueueMap::<T>::new(),
            threads: ThreadMap::new(),
            shutdown_lock: Some(lock),
            stopped: false,
            limit: limit,
        }
    }

    /// Add a task to a queue. When the task is complete it will be sent back
    /// over the returned `Receiver`.
    ///
    /// # Failures
    ///
    /// - `QueueMissing`: Could not find the queue. This will only happen if an
    /// [`add_task()`](#method.add_task) call happens before an
    /// [`ensure_queue()`](#method.ensure_queue) call for that queue.
    ///
    /// - `Shutdown`: Manager is not accepting new tasks at the moment. This
    /// will happen if an [`add_task()`](#method.add_task) call happens after a
    /// [`shutdown()`](#method.shutdown) but before a
    /// [`restart()`](#method.restart).
    pub fn add_task(&mut self, queue_key: &QueueKey, task: T) -> Result<Receiver<T>, Error> {
        if self.stopped {
            return Err(Error::Shutdown);
        }
        let (task_tx, task_rx) = channel();
        {
            let mut locked_queue = match self.queues.get_mut(queue_key) {
                // Safe unwrap: With the current implementation it's impossible
                // for a lock to get poisoned. There is exactly one other spot
                // where we acquire a queue lock, in the worker thread, and the
                // only thing we do is `pop_task`, an alias for `pop_front` on
                // the underlying VecDeque, which cannot cause a thread
                // panic. In this method we only do one thing while holding the
                // lock, `push_task` (an alias for `push_back`) which also
                // cannot cause a thread panic.
                Some(queue_mutex) => queue_mutex.lock().unwrap(),
                None => return Err(Error::QueueMissing),
            };
            locked_queue.push_task((task, task_tx));
        }

        // Safe unwrap: If the queue exists, a corresponding thread in the
        // thread map is guaranteed to exist because both maps are private and
        // we always create a thread map entry when we create a queue map entry.
        let worker_tx = self.get_channel(queue_key).unwrap();

        // Safe unwrap: The worker thread doesn't perform any operations that
        // can cause a panic.
        worker_tx.send(()).unwrap();

        Ok(task_rx)
    }

    #[allow(unused_must_use)]
    /// Signal worker threads to shut down after any active tasks and once they
    /// are all done send a signal down the shutdown lock channel. Trying to add
    /// tasks after calling [`shutdown()`](#method.shutdown) but before calling
    /// [`restart()`](#method.restart) will result in an `Error::Shutdown`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::thread;
    /// # use std::sync::mpsc::channel;
    /// # use std::sync::{Arc, Mutex};
    /// # use hookshot::task_manager::{TaskManager, Runnable, Error};
    /// # struct ImportantTask;
    /// # impl Runnable for ImportantTask {
    /// #     fn run(&mut self) { println!("task added") }
    /// # }
    /// # let important_task1 = ImportantTask;
    /// # let important_task2 = ImportantTask;
    /// let (shutdown_tx, shutdown_rx) = channel();
    /// let tasks = Arc::new(Mutex::new(TaskManager::new_with_lock(None, shutdown_tx)));
    ///
    /// let shared_tasks = tasks.clone();
    /// thread::spawn(move || {
    ///     let mut tasks = shared_tasks.lock().unwrap();
    ///     let queue = tasks.ensure_queue(String::from("$"));
    ///     tasks.add_task(&queue, important_task1);
    /// });
    ///
    /// // Wait 100ms then signal a shutdown for task manager.
    /// let shared_tasks = tasks.clone();
    /// thread::spawn(move || {
    ///     thread::sleep_ms(100);
    ///     let mut tasks = shared_tasks.lock().unwrap();
    ///     tasks.shutdown();
    /// });
    ///
    /// // Wait 150ms then try to add a task. This will fail because the task
    /// // manager will have received a `shutdown()` call by this point.
    /// let shared_tasks = tasks.clone();
    /// thread::spawn(move || {
    ///     thread::sleep_ms(150);
    ///     let mut tasks = shared_tasks.lock().unwrap();
    ///     let queue = tasks.ensure_queue(String::from("$"));
    ///     match tasks.add_task(&queue, important_task2) {
    ///          Err(Error::Shutdown) => println!("queue was shut down"),
    ///          // ...
    /// #        _ => unreachable!()
    ///     };
    /// });
    ///
    /// // Block thread until shutdown is complete.
    /// shutdown_rx.recv();
    ///
    /// println!("all workers stopped");
    /// ```
    pub fn shutdown(&mut self) {
        self.stopped = true;
        for key in self.queues.keys() {
            // Remove thread join handle from threadmap, letting worker_tx drop
            // out of scope so the worker thread quits instead of picking a new
            // task then wait for the thread to finish.
            let handle = {
                match self.threads.remove(key) {
                    Some((handle, _)) => handle,
                    None => continue,
                }
            };
            handle.join();
        }
        if let Some(ref tx) = self.shutdown_lock {
            tx.send(());
        }
    }

    /// Restart all queue workers and remove `stopped` flag.
    pub fn restart(&mut self) {
        let keys: Vec<_> = self.queues.keys().cloned().collect();
        for key in keys {
            self.start_worker(key);
        }
        self.stopped = false;
    }

    /// Create a queue only if one doesn't already exist with that key. Returns
    /// the QueueKey for that queue.
    pub fn ensure_queue(&mut self, queue_key: String) -> QueueKey {
        let key = QueueKey { k: queue_key };
        if self.queues.contains_key(&key) {
            return key;
        }

        let queue = Arc::new(Mutex::new(Queue::<T>::new(self.limit)));
        self.queues.insert(key.clone(), queue);
        self.start_worker(key.clone());
        key
    }

    fn find(&mut self, key: &QueueKey) -> Option<&mut Arc<Mutex<Queue<T>>>> {
        self.queues.get_mut(key)
    }

    fn get_channel(&self, key: &QueueKey) -> Option<&Sender<()>> {
        match self.threads.get(key) {
            Some(&(_, ref tx)) => Some(tx),
            None => None,
        }
    }

    #[allow(unused_must_use)]
    fn start_worker(&mut self, key: QueueKey) {
        if self.stopped {
            return;
        }
        if self.threads.contains_key(&key) {
            return;
        }

        let queue = self.find(&key).unwrap().clone();
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
            }
        });
        self.threads.insert(key, (worker, worker_tx));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    struct Task {
        s: Arc<Mutex<String>>,
        m: &'static str,
    }

    impl Runnable for Task {
        fn run(&mut self) {
            let mut s = self.s.lock().unwrap();
            s.push_str(self.m);
            thread::sleep_ms(50);
        }
    }

    #[test]
    #[allow(unused_must_use)]
    fn test_task_manager() {
        let s1 = Arc::new(Mutex::new(String::new()));
        let s2 = Arc::new(Mutex::new(String::new()));

        let task_manager = Arc::new(Mutex::new(TaskManager::new(None)));
        let thread1 = {
            let shared_manager = task_manager.clone();
            let s = s1.clone();
            thread::spawn(move || {
                let mut manager = shared_manager.lock().unwrap();
                let key = Uuid::new_v4().to_string();

                let queue_key = manager.ensure_queue(key);
                manager.add_task(&queue_key, Task {s: s.clone(), m: "b"});
                manager.add_task(&queue_key, Task {s: s.clone(), m: "r"});
                manager.add_task(&queue_key, Task {s: s.clone(), m: "i"});
                manager.add_task(&queue_key, Task {s: s.clone(), m: "a"});
                manager.add_task(&queue_key, Task {s: s.clone(), m: "n"})
                    .unwrap().recv();

            })
        };
        let thread2 = {
            let shared_manager = task_manager.clone();
            let s = s2.clone();
            thread::spawn(move || {
                let mut manager = shared_manager.lock().unwrap();
                let key = Uuid::new_v4().to_string();

                let queue_key = manager.ensure_queue(key);
                manager.add_task(&queue_key, Task { s: s.clone(), m: "s", });
                manager.add_task(&queue_key, Task { s: s.clone(), m: "l", });
                manager.add_task(&queue_key, Task { s: s.clone(), m: "o", });
                manager.add_task(&queue_key, Task { s: s.clone(), m: "t", });
                manager.add_task(&queue_key, Task { s: s.clone(), m: "h", });
                manager.add_task(&queue_key, Task { s: s.clone(), m: "s", })
                    .unwrap().recv();
            })
        };

        thread1.join();
        thread2.join();

        assert_eq!(*s1.lock().unwrap(), "brian");
        assert_eq!(*s2.lock().unwrap(), "sloths");
    }

    #[test]
    #[allow(unused_must_use)]
    fn test_task_manager_limit() {
        let s1 = Arc::new(Mutex::new(String::new()));
        let limit = Some(1);
        let task_manager = Arc::new(Mutex::new(TaskManager::new(limit)));

        let thread1 = {
            let shared_manager = task_manager.clone();
            let s = s1.clone();
            thread::spawn(move || {
                let mut manager = shared_manager.lock().unwrap();
                let key = Uuid::new_v4().to_string();

                let queue_key = manager.ensure_queue(key);
                manager.add_task(&queue_key, Task {s: s.clone(), m: "1"});
                thread::sleep_ms(1);
                manager.add_task(&queue_key, Task {s: s.clone(), m: "2"});
                thread::sleep_ms(1);
                manager.add_task(&queue_key, Task {s: s.clone(), m: "3"});
                thread::sleep_ms(1);
                manager.add_task(&queue_key, Task {s: s.clone(), m: "4"});
                thread::sleep_ms(1);
                manager.add_task(&queue_key, Task {s: s.clone(), m: "5"})
                    .unwrap().recv();

            })
        };
        thread1.join();

        assert_eq!(*s1.lock().unwrap(), "15");
    }

}
