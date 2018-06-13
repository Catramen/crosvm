
use std::thread;
use std::sync::mpsc;
use std::rc::Rc;

type IOThreadTask = Box<Fn() + Send >;

enum IOThreadEvents {
    Stop,
    RunTask(IOThreadTask),
}

#[derive(Clone)]
pub struct IOThread {
    sender_channel: mpsc::Sender<IOThreadEvents>,
}


impl IOThread {
    pub fn start() -> (IOThread, thread::JoinHandle<()>) {
        let (sender, receiver) = mpsc::channel::<IOThreadEvents>();
        let handle = thread::spawn(move || {
            loop {
                let event = match receiver.recv() {
                    Ok(ev) => {
                        ev
                    },
                    Err(_e) => return,
                };

                match event {
                    IOThreadEvents::Stop => return,
                    IOThreadEvents::RunTask(t) => t(),
                }
            }
        });
        (
            IOThread {
                sender_channel: sender,
            },
            handle
        )
    }

    pub fn post_task<T: Fn() + Send + 'static> (&self, t: T) {
        self.sender_channel.send(IOThreadEvents::RunTask(Box::new(t)));
    }

    pub fn stop(&self) {
        self.sender_channel.send(IOThreadEvents::Stop);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex, Condvar};

    fn set_to_101(v: &mut u8) {
        *v = 101;
    }

    #[test]
    fn test_basic_post_task() {
        let (io, join) = IOThread::start();
        let data = Arc::new(Mutex::new(0u8));
        let d2 = data.clone();
        io.post_task(move || {
            set_to_101(&mut (d2.lock().unwrap()));
        });
        io.stop();
        join.join();
        assert_eq!(*data.lock().unwrap(), 101);
    }

    #[test]
    fn test_multisource_post_task() {
        let (io, join) = IOThread::start();
        let pair = Arc::new((Mutex::new(false), Condvar::new()));
        let pair2 = pair.clone();
        let data = Arc::new(Mutex::new(0u8));
        let io2 = io.clone();
        let d2 = data.clone();
        io.post_task(move || {
            let d3 = d2.clone();
            let pair3 = pair2.clone();
            set_to_101(&mut (d2.lock().unwrap()));
            io2.post_task(move || {
                (*d3.lock().unwrap()) = 10;
                let &(ref lock, ref cvar) = &*pair3;
                let mut finished = lock.lock().unwrap();
                *finished = true;
                cvar.notify_one();
            });
        });
        let &(ref lock, ref cvar) = &*pair;
        let mut finished = lock.lock().unwrap();
        while !*finished {
            finished = cvar.wait(finished).unwrap();
        }
        io.stop();
        join.join();
        assert_eq!(*data.lock().unwrap(), 10);
    }
}
