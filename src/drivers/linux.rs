use libc::timeval;
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::sync::mpsc::{channel, Receiver, Sender};

use crate::Key;
use crate::{InputDriver, OutputDriver};
use std::os::unix::io::AsRawFd;

#[repr(C)]
struct LinuxEvt {
    time: timeval,
    evt_type: u16,
    code: u16,
    value: i32,
}

#[repr(C)]
struct LinuxInputId {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

#[repr(C)]
pub struct LinuxUSetup {
    id: LinuxInputId,
    name: [u8; 80],
    ff_effects_max: u32,
}

pub struct LinuxInputDriver {
    // Event output
    sender: Sender<Key>,
    reader: BufReader<File>,
}

impl InputDriver for LinuxInputDriver {
    fn new(sender: Sender<Key>) -> Self {
        let input = File::open("/dev/input/event0").unwrap();
        let reader = BufReader::new(input);
        Self { sender, reader }
    }

    fn run(&mut self) -> () {
        loop {
            let evt = read_struct::<LinuxEvt, BufReader<File>>(&mut self.reader);
            match evt {
                Ok(e) => {
                    if e.evt_type == 1 && (e.value == 1 || e.value == 2) {
                        self.sender.send(e.code).unwrap();
                    }
                }
                Err(_) => println!("Err"),
            }
        }
    }
}

pub struct LinuxOutputDriver {
    sender: Sender<Key>,
    receiver: Receiver<Key>,
    writer: BufWriter<File>,
}

// TODO: move
fn write_struct<T: Sized, W: Write>(p: &T, buf: &mut W) -> () {
    unsafe {
        let bytes: &[u8] =
            ::std::slice::from_raw_parts((p as *const T) as *const u8, ::std::mem::size_of::<T>());
        buf.write(bytes).unwrap();
        buf.flush().unwrap();
    }
}

// TODO: move
fn read_struct<T, R: Read>(read: &mut R) -> io::Result<T> {
    let num_bytes = ::std::mem::size_of::<T>();
    unsafe {
        let mut s = ::std::mem::uninitialized();
        let buffer = ::std::slice::from_raw_parts_mut(&mut s as *mut T as *mut u8, num_bytes);
        match read.read_exact(buffer) {
            Ok(()) => Ok(s),
            Err(e) => {
                ::std::mem::forget(s);
                Err(e)
            }
        }
    }
}

impl LinuxOutputDriver {
    ioctl_write_int!(set_evbit, 'U', 100);
    ioctl_write_int!(set_keybit, 'U', 101);
    ioctl_write_ptr!(dev_setup, 'U', 3, LinuxUSetup);
    ioctl_none!(dev_create, 'U', 1);

    fn setup(file: &mut File) -> () {
        let fd = file.as_raw_fd();
        let name: [u8; 80] = [30; 80];
        let setup = LinuxUSetup {
            id: LinuxInputId {
                bustype: 3,
                vendor: 0x1234,
                product: 0x5678,
                version: 0,
            },
            name: name,
            ff_effects_max: 0,
        };

        unsafe {
            Self::set_evbit(fd, 1).unwrap();
            for i in 1..249 {
                Self::set_keybit(fd, i).unwrap();
            }
            Self::dev_setup(fd, &setup).unwrap();
            Self::dev_create(fd).unwrap();
        }
    }
}

impl OutputDriver for LinuxOutputDriver {
    fn new() -> Self {
        let (sender, receiver) = channel();
        let mut output = OpenOptions::new().write(true).open("/dev/uinput").unwrap();
        Self::setup(&mut output);
        let writer = BufWriter::new(output);
        Self {
            sender,
            receiver,
            writer,
        }
    }

    fn get_sender(&self) -> Sender<Key> {
        self.sender.clone()
    }

    fn run(&mut self) -> () {
        loop {
            match self.receiver.recv() {
                Ok(c) => {
                    let time = timeval {
                        tv_sec: 0,
                        tv_usec: 0,
                    };
                    let syn = LinuxEvt {
                        time: time,
                        evt_type: 0,
                        code: 0,
                        value: 0,
                    };
                    let evt1 = LinuxEvt {
                        time: time,
                        evt_type: 1,
                        code: c,
                        value: 1,
                    };
                    let evt2 = LinuxEvt {
                        time: time,
                        evt_type: 1,
                        code: c,
                        value: 0,
                    };
                    write_struct(&evt1, &mut self.writer);
                    write_struct(&syn, &mut self.writer);
                    write_struct(&evt2, &mut self.writer);
                    write_struct(&syn, &mut self.writer);
                    println!("Replaying {}", c)
                }
                Err(_) => println!("Err"),
            }
        }
    }
}
