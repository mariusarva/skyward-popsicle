//! Opening the data partition and its users' own files on a thread of its
//! own, so the screen keeps up: the keys come from the secure world, and
//! what a user enters at their screen lock is checked slowly on purpose.

use anyhow::Result;
use skyward_binder::driver::Binder;
use skyward_crypto::data::{self, Closed, Opening};
use skyward_crypto::keymint::KeyMint;
use skyward_sys::worker::Worker;
use std::os::fd::RawFd;

pub enum Job {
    /// Open the partition as far as it opens with nothing a user knows.
    Open,
    /// Open the own files of `user` with what they entered at their screen
    /// lock; `closed` are the users whose own files are still closed.
    Unlock { closed: Vec<Closed>, user: u32, credential: Vec<u8> },
}

/// How a job ended: what it opened, or why it did not.
pub enum Ended {
    Opened(Result<Opening, String>),
    Unlocked(Result<Vec<Closed>, String>),
}

/// A job done on a thread of its own, which the screen lets go once the
/// job has ended.
pub struct Decryptor {
    /// Tells how the job ended, once.
    worker: Worker<Ended>,
}

impl Decryptor {
    pub fn start(job: Job) -> Result<Decryptor> {
        // Which of the two answers the page waits for, so that work that
        // comes apart is told the same way a refused one is.
        let unlock = matches!(job, Job::Unlock { .. });
        let worker = Worker::start("decrypt", move |tell| {
            // Work that comes apart is told as why it failed rather than left
            // to end the process: init would start the recovery again and the
            // page would come back to the same place (2026-11-19).
            let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match job {
                Job::Open => Ended::Opened(with_key_service(data::open_without_credential)),
                Job::Unlock { closed, user, credential } => {
                    Ended::Unlocked(with_key_service(|service| data::unlock(service, &closed, user, &credential)))
                }
            }));
            tell.tell(match done {
                Ok(ended) => ended,
                Err(panic) => {
                    let why = format!("the work came apart: {}", crate::ui::panic_text(&*panic));
                    let _ = skyward_sys::kmsg(&why);
                    match unlock {
                        true => Ended::Unlocked(Err(why)),
                        false => Ended::Opened(Err(why)),
                    }
                }
            });
        })?;
        Ok(Decryptor { worker })
    }

    /// Readable once the job has ended.
    pub fn fd(&self) -> RawFd {
        self.worker.fd()
    }

    /// How the job ended, once it has.
    pub fn ended(&self) -> Option<Ended> {
        self.worker.events().pop()
    }

    /// Stop waiting on the job without ending it: the thread goes on and what
    /// it tells is dropped. For a page that has decided to ask the user again
    /// rather than wait on a secure world that may hold the answer a minute.
    pub fn detach(self) {
        self.worker.detach();
    }
}

/// Do `work` with the phone's key service, reached afresh for it, and say
/// why it failed in words for the page.
pub fn with_key_service<T>(work: impl FnOnce(&KeyMint) -> Result<T>) -> Result<T, String> {
    let done = (|| {
        let binder = Binder::open()?;
        let service = KeyMint::open(&binder)?;
        work(&service)
    })();
    done.map_err(|e| format!("{e:#}"))
}
