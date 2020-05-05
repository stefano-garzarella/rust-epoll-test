# rust-epoll-test

VSOCK wrapper library for Rust

## Usage

```bash
> ./rust-epoll-test -h
rust-epoll-test 0.1.0
Stefano Garzarella <sgarzare@redhat.com>
rust epoll stress test

USAGE:
    rust-epoll-test [FLAGS] [OPTIONS] <uds_path> --client --server

FLAGS:
    -c, --client     client mode
    -h, --help       Prints help information
    -s, --server     server mode
    -V, --version    Prints version information

OPTIONS:
    -l, --len <buf_len>    length of buffer to read or write [default: 4KiB]
    -t, --time <time>      time in seconds to transmit for (0 means infinite)

ARGS:
    <uds_path>    uds path
```

## Example


### Server

Server with 64KiB write buffer size

```bash
> ./rust-epoll-test -s /tmp/uds.sock -l 64KiB
UDS: /tmp/uds.sock buf_len: 65536
waiting clients...
client connected
writing... [64.255 GB - 9.000 s - 61.325 Gbps]
connection closed
written 79540322304 bytes in 9.999941116 seconds
waiting clients...
```

### Client

Client with 64KiB read buffer size and test running for 10 seconds
```bash
> ./rust-epoll-test -c /tmp/uds.sock -l 64KiB -t 10
UDS: /tmp/uds.sock buf_len: 65536
connecting to the server..
connected
reading... [64.254 GB - 9.000 s - 61.324 Gbps]
read 79540256768 bytes in 10.000001991 seconds
```
