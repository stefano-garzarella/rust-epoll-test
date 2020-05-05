use clap::{Arg, App, crate_authors, crate_version};
use std::os::unix::net::{UnixListener, UnixStream};
use std::io::{self, ErrorKind};
use std::io::prelude::*;
use std::os::unix::io::{AsRawFd, RawFd};
use std::time::Instant;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

extern crate byte_unit;
use byte_unit::{Byte};

#[macro_use] extern crate log;

const PRINT_TIMEOUT: u64 = 1;

fn gbytes(bytes: u64) -> f64 {
    return (bytes as f64) / 1024.0 / 1024.0 / 1024.0;
}

fn throughput(start: &Instant, bytes: u64) -> f64 {
    let bits = (bytes as f64) / 1e9 * 8.0;

    return bits / start.elapsed().as_secs_f64();
}

fn server_handle_connection(sock: &mut UnixStream, buf_len: usize) {
    let mut buf: Vec<u8> = Vec::with_capacity(buf_len);
    let epoll_fd = epoll::create(true).unwrap();
    let sock_fd = sock.as_raw_fd();
    let mut evset = epoll::Events::empty();
    let mut written : u64 = 0;

    unsafe {buf.set_len(buf_len)};

    sock.set_nonblocking(true).unwrap();

    evset.insert(epoll::Events::EPOLLIN);
    evset.insert(epoll::Events::EPOLLOUT);
    evset.insert(epoll::Events::EPOLLHUP);
    evset.insert(epoll::Events::EPOLLRDHUP);

    epoll::ctl(
        epoll_fd,
        epoll::ControlOptions::EPOLL_CTL_ADD,
        sock_fd,
        epoll::Event::new(evset, sock_fd as u64),
    ).unwrap();

    let mut epoll_events =
        vec![epoll::Event::new(epoll::Events::empty(), 0); 32];

    let start = Instant::now();
    let mut now = Instant::now();

    'epoll: loop {
        match epoll::wait(epoll_fd, 0, epoll_events.as_mut_slice()) {
            Ok(ev_cnt) => {
                #[allow(clippy::needless_range_loop)]
                for i in 0..ev_cnt {
                    let fd = epoll_events[i].data as RawFd;
                    let evset = epoll::Events::from_bits(epoll_events[i].events).unwrap();

                    if fd != sock_fd {
                        error!("Wrong fd {} [expected {}]", fd, sock_fd);
                    }

                    if evset.contains(epoll::Events::EPOLLHUP) ||
                        evset.contains(epoll::Events::EPOLLRDHUP) ||
                        evset.contains(epoll::Events::EPOLLERR) {
                        println!("\nConnection closed");
                        break 'epoll;
                    }

                    if evset.contains(epoll::Events::EPOLLOUT) {
                        match sock.write(&buf) {
                            Ok(cnt) => {
                                written += cnt as u64;

                                if now.elapsed().as_secs() >= PRINT_TIMEOUT {

                                    print!("\rwriting... [{:.3} GB - {:.3} s - {:.3} Gbps]",
                                           gbytes(written),
                                           start.elapsed().as_secs_f64(),
                                           throughput(&start, written));

                                    io::stdout().flush().unwrap();
                                    now = Instant::now();
                                }
                            }
                            Err(err) => {
                                if err.kind() == ErrorKind::WouldBlock {
                                    println!("\nWouldBlock");
                                } else {
                                    error!("Write failed: {}", err);
                                }
                            }
                        };
                    }

                    if evset.contains(epoll::Events::EPOLLIN) {
                        println!("\nEPOLLIN");
                    }

                }
            }
            Err(e) => {
                warn!("epoll wait failed: {}", e);
                break 'epoll;
            }
        }
    }

    println!("Written {} bytes in {} seconds",
              written, start.elapsed().as_secs_f64());
}

fn server(uds_path: &str, buf_len: usize) {
    println!("server");


    let listener = UnixListener::bind(uds_path).unwrap();

    println!("Waiting clients...");

    for sock in listener.incoming() {
        match sock {
            Ok(mut sock) => {

                println!("client connected");

                server_handle_connection(&mut sock, buf_len);

                println!("waiting clients...");
            }
            Err(err) => {
                error!("connection failed: {}", err);
                break;
            }
        }
    }

}

fn client(uds_path: &str, buf_len: usize) {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    println!("Connecting to the server..");

    let mut read : u64 = 0;

    let mut sock = UnixStream::connect(uds_path).unwrap();

    println!("Connected");

    let mut buf: Vec<u8> = Vec::with_capacity(buf_len);
    unsafe {buf.set_len(buf_len)};

    let start = Instant::now();
    let mut now = Instant::now();

    while running.load(Ordering::SeqCst) {
        match sock.read(&mut buf) {
            Ok(cnt) => {
                if cnt == 0 {
                    break;
                } else {
                    read += cnt as u64;
                    if now.elapsed().as_secs() >= PRINT_TIMEOUT {
                        print!("\rreading... [{:.3} GB - {:.3} s - {:.3} Gbps]",
                               gbytes(read),
                               start.elapsed().as_secs_f64(),
                               throughput(&start, read));

                        io::stdout().flush().unwrap();
                        now = Instant::now();
                    }
                }
            }
            Err(err) => {
                error!("Read failed: {}", err);
            }
        };
    }

    println!("\nRead {} bytes in {} seconds",
              read, start.elapsed().as_secs_f64());
}


fn main() {

    let cmd_args = App::new("rust-epoll-test")
        .version(crate_version!())
        .author(crate_authors!())
        .about("rust epoll stress test")
        .arg(
            Arg::with_name("server")
                .long("server")
                .short("s")
                .conflicts_with("client")
                .required(true)
                .help("server mode"),
        )
        .arg(
            Arg::with_name("client")
                .long("client")
                .short("c")
                .conflicts_with("server")
                .required(true)
                .help("client mode"),
        )
        .arg(
            Arg::with_name("uds_path")
                .required(true)
                .help("uds path"),
        )
        .arg(
            Arg::with_name("buf_len")
                .long("len")
                .short("l")
                .default_value("4KiB")
                .help("length of buffer to read or write"),
        )
        .get_matches();

    let uds_path = cmd_args.value_of("uds_path").unwrap();
    let buf_len = Byte::from_str(cmd_args.value_of("buf_len").unwrap()).unwrap();

    println!("UDS: {} buf_len: {}", uds_path, buf_len);

    if cmd_args.is_present("server") {
        server(uds_path, buf_len.get_bytes() as usize);
    } else {
        client(uds_path, buf_len.get_bytes() as usize);
    }
}
