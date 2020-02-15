use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::Instant;

#[macro_use(ioctl_write_int, ioctl_write_ptr, ioctl_none)]
extern crate nix;
mod drivers;
use crate::drivers::linux::{LinuxInputDriver, LinuxOutputDriver};

/*
 * Should be able to
 * - produce symbol-strings as sent by user
 * - receive symbol-strings to be sent to OS
 * - (detect context-switching?)
 * - (detect current layout?)
 * - (have configuration dangling out?)
*/

type Key = u16;

trait InputDriver {
    // Create a new instance with given sender to send events through
    fn new(sender: Sender<Key>) -> Self;
    fn run(&mut self) -> ();
}

trait OutputDriver {
    fn new() -> Self;
    fn get_sender(&self) -> Sender<Key>;
    fn run(&mut self) -> ();
}

fn main() {
    let (s, r) = channel();

    let mut input = LinuxInputDriver::new(s);
    let mut output = LinuxOutputDriver::new();
    let output_sender = output.get_sender();

    let a = thread::spawn(move || {
        input.run();
    });

    let c = thread::spawn(move || {
        output.run();
    });

    let b = thread::spawn(move || {
        let mut hist = Vec::new();
        let mut last_hit = Instant::now();

        loop {
            match r.recv() {
                Ok(v) => {
                    println!("Key {}", v);
                    if v == 119 {
                        println!("Replay!");
                        for key in &hist {
                            output_sender.send(*key).unwrap();
                        }
                        hist.clear();
                    } else {
                        if last_hit.elapsed().as_secs() > 2 {
                            hist.clear();
                        }
                        last_hit = Instant::now();
                        hist.push(v);
                    }
                }

                Err(_) => (),
            }
        }
    });

    a.join().unwrap();
    c.join().unwrap();
    b.join().unwrap();

    println!("Hello, world!");
}
