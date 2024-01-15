#[cfg(feature="multithreaded")]
pub mod multithreaded {
    use std::sync::{mpsc, Arc, Mutex};
    use std::thread;

    type Job = Box<dyn FnOnce() + Send + 'static>;

    pub struct PoolCreationError {}

    pub struct ThreadPool {
        workers: Vec<Worker>,
        sender: Option<mpsc::Sender<Job>>,
    }


    impl ThreadPool {
        /// Builds a thread pool
        ///
        /// size is how many threads to spawn
        pub fn build(size: usize) -> Result<ThreadPool, PoolCreationError> {
            if size < 1 {
                Err(PoolCreationError {})
            } else {
                let (sender, receiver) = mpsc::channel();
                let receiver = Arc::new(Mutex::new(receiver));
                let mut workers = Vec::with_capacity(size);
                for id in 0..size {
                    workers.push(Worker::new(id, Arc::clone(&receiver)));
                }
                Ok(ThreadPool {
                    workers,
                    sender: Some(sender),
                })
            }
        }
        /// Executes a job
        pub fn execute<F>(&self, f: F)
        where
            F: FnOnce() + Send + 'static,
        {
            let job = Box::new(f);

            self.sender.as_ref().unwrap().send(job).unwrap();
        }
    }

    #[cfg(feature = "multithreaded")]
    impl Drop for ThreadPool {
        fn drop(&mut self) {
            drop(self.sender.take());
            for worker in &mut self.workers {
                println!("Shutting down worker {}", worker.id);
                if let Some(thread) = worker.thread.take() {
                    thread.join().unwrap();
                }
            }
        }
    }

    #[cfg(feature = "multithreaded")]
    struct Worker {
        id: usize,
        thread: Option<thread::JoinHandle<()>>,
    }

    #[cfg(feature = "multithreaded")]
    impl Worker {
        fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
            let thread = thread::spawn(move || loop {
                let message = receiver.lock().unwrap().recv();
                match message {
                    Ok(job) => {
                        println!("Worker {id} got a job; executing.");
                        job();
                    }
                    Err(_) => {
                        println!("Worker {id} disconnected; shutting down.");
                        break;
                    }
                }
            });
            Worker {
                id,
                thread: Some(thread),
            }
        }
    }
}
