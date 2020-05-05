use clap::{Arg, App, crate_authors, crate_version};
use std::os::unix::net::{UnixListener, UnixStream};
use std::io::{ErrorKind};
use std::io::prelude::*;
use std::os::unix::io::{AsRawFd, RawFd};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[macro_use] extern crate log;

fn server_handle_connection(sock: &mut UnixStream) {
    let epoll_fd = epoll::create(true).unwrap();
    let sock_fd = sock.as_raw_fd();
    let mut written : u64 = 0;
    let mut evset = epoll::Events::empty();
    let buf_len = 64 * 1024;
    let mut buf: Vec<u8> = Vec::with_capacity(buf_len);
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
                        println!("ending");
                        break 'epoll;
                    }

                    if evset.contains(epoll::Events::EPOLLOUT) {
                        match sock.write(&buf) {
                            Ok(cnt) => {
                                written += cnt as u64;
                            }
                            Err(err) => {
                                if err.kind() == ErrorKind::WouldBlock {
                                    println!("WouldBlock");
                                } else {
                                    println!("write failed: {}", err);
                                }
                            }
                        };
                    }

                    if evset.contains(epoll::Events::EPOLLIN) {
                        println!("EPOLLIN");
                    }

                }
            }
            Err(e) => {
                warn!("epoll wait failed: {}", e);
                break 'epoll;
            }
        }
    }

    println!("written {} bytes", written);
}
fn server(uds_path: &str) {
    println!("server");


    let listener = UnixListener::bind(uds_path).unwrap();

    for sock in listener.incoming() {
        match sock {
            Ok(mut sock) => {

                println!("client connected...");

                server_handle_connection(&mut sock);
            }
            Err(err) => {
                error!("connection failed: {}", err);
                break;
            }
        }
    }

}

fn client(uds_path: &str) {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");

    println!("client");
    let buf_len = 1024;
    let mut read : u64 = 0;

    let mut sock = UnixStream::connect(uds_path).unwrap();

    let mut buf: Vec<u8> = Vec::with_capacity(buf_len);
    unsafe {buf.set_len(buf_len)};

    while running.load(Ordering::SeqCst) {
        match sock.read(&mut buf) {
            Ok(cnt) => {
                if cnt == 0 {
                    break;
                } else {
                    read += cnt as u64;
                }
            }
            Err(err) => {
                error!("read failed: {}", err);
            }
        };
    }

    println!("read {} bytes", read);
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
        .get_matches();

    let uds_path = cmd_args.value_of("uds_path").unwrap();

    println!("UDS: {}", uds_path);

    if cmd_args.is_present("server") {
        server(uds_path);
    } else {
        client(uds_path);
    }
}
