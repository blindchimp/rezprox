// This program implements an app-specific network relay.
//
// This process is used to relay control connections and audio and video
// data streams between dwyco applications. It is used in situations
// where the two client applications cannot connect directly to each
// other.
//
// Normally, this process is invoked by something like xinetd at the
// request of a local server that is assisting in connecting two clients.
// This process allocates some sockets, and relays the port information
// to the requesting server (on socket 0, if invoked by xinetd.)
// The server then relays the port information to the clients.
// The clients then initiate connections directly to this relay, which
// sets up clear channels between the sockets, and simply shovels
// data between the sockets.
// Multiple connections are assumed, the first is a "control connection"
// which is treated specially. If there are any errors or timeouts on
// this connection, the entire relay process is terminated.
// Other subsequent connections are treated more leniently, since they
// are expected to come and go as a normal part of the client
// communications. This process terminates after 1 hour, as a safety
// measure.
//
// This proxy must be invoked with 2 arguments:
// rezprox -c <dir>
//
// the proxy must be able to chdir to the directory specified.
//
// the proxy reads the IP address of the interface to bind on from
// a file called "cfg/HostIP". if it cannot open this file, it uses
// the IP address "127.0.0.1".
//
// after successfully binding and listening on two arbitrary ports
// provided by the OS, the proxy sends a xfer-rep formatted string
// on file descriptor 0 (which xinetd has connected to a client.)
// the client can forward that information to parties that want to
// rendevous with each other.
//
// if no control connections are created in the first 30 seconds
// of running, it exits.
// the proxy dies after an hour, regardless of any connections it has.
//
// Dwyco, Inc.
// Feb, 2022
//
// this is my first experience with rust... just a note:
// Unless I'm missing something, rust std libs seems to be lacking a few things
// that would be very handy for this sort of proxy:
// * being able to split a tcp stream into read-side and write-side.
// * being able to select on multiple streams at the same time
// * accept connections from a tcp stream with a timeout
//
// it appears that most of this functionality is available in third-party
// crates like tokio and crossbeam

use log::info;
use std::env;
use std::fs;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::net::Shutdown;
use std::net::TcpListener;
use std::net::TcpStream;
use std::os::unix::io::FromRawFd;
use std::path::Path;
use std::process;
use std::thread;
use std::time::Duration;

use crossbeam::channel::unbounded;
use crossbeam::channel::{Receiver, Sender};

// simple watchdog
//
// this should be run in a thread. ticks are in
// seconds. it simply counts down until it
// reaches 0 and exits the entire process.
//
// it receives commands on the cmd receiver:
// < 0 : exits the thread
// 0 : don't send this, used internally
// anything else is interpreted as a new
// timer value (in seconds), which is installed immediately.
//
fn watchdog(cmd: Receiver<i32>, init_ticks: i32) {
    let mut ticks_left = init_ticks;
    loop {
        if ticks_left == 0 {
            process::exit(1);
        }
        let new_ticks = cmd.recv_timeout(Duration::from_secs(1)).unwrap_or(0);
        if new_ticks == 0 {
            ticks_left -= 1;
            info!("TIMEOUT {}", ticks_left);
        } else if new_ticks < 0 {
            info!("stop");
            break;
        } else {
            ticks_left = new_ticks;
            info!("reset {}", new_ticks);
        }
    }
}

// this resets the watchdog timeout to new_ticks
// the sender has to be associated with the
// receiver the watchdog thread is using.
fn reset(sender: &Sender<i32>, new_ticks: i32) {
    sender.try_send(new_ticks).unwrap();
}

// faux encode an integer, for compat with existing
// services that use the xfer protocol
fn xenc(i: usize) -> String {
    let istr = i.to_string();
    let len = istr.len();
    format!("{:02}{}", len, istr)
}

// shovel data from the input stream to output stream.
// on error, if die_hard is true, just quit the process.
// otherwise, just shutdown the connections and terminate the thread.
//
// timeouts: if die_hard is true, a short timeout is used because
// we know the protocol on that set of connections is pinging at
// regular intervals.
//
// note: it is assumed that shoveling threads are created in
// pairs, with the order of the inp and output swapped.
// some of the cloning that goes on might be avoidable if it
// was possible to split the input and output sides of the
// streams (i think tokio allows this?)

fn shovel(inp: TcpStream, mut outp: TcpStream, die_hard: bool) {
    let dead_inp = inp.try_clone().expect("can't clone in shovel");
    let mut buf = BufReader::new(inp);
    let (shovel_wd_tx, shovel_wd_cmd) = unbounded();
    if die_hard {
        thread::spawn(|| {
            watchdog(shovel_wd_cmd, 3 * 60);
        });
    }

    loop {
        if die_hard {
            reset(&shovel_wd_tx, 3 * 60);
        }
        // if this unwrap fails, does it zap the thread? or the
        // whole process?
        let bytes = buf.fill_buf().unwrap();
        let len_read = bytes.len();
        if len_read == 0 {
            if die_hard {
                process::exit(0);
            } else {
                // note: we know there is another
                // shovel thread using our same set of
                // connections, just in the opposite order,
                // so closing them in one thread will result
                // in errors in the other side, causing the
                // proxy connection to shutdown completely
                let _ = dead_inp.shutdown(Shutdown::Both);
                let _ = outp.shutdown(Shutdown::Both);
                break;
            }
        }
        outp.write_all(bytes).unwrap();
        buf.consume(len_read);
    }
}

/*
fn
rendevous_forever(sock: TcpListener, outchan: SyncSender<TcpStream>) {
    loop {
        let (strm, _) = sock.accept().unwrap();
        outchan.send(strm).unwrap();
    }
}
*/

fn main() {
    env_logger::init();

    // watchdog for setting up rendevous
    let (startup_wd_tx, wd_rx) = unbounded();
    thread::spawn(|| {
        watchdog(wd_rx, 4);
    });

    let args: Vec<String> = env::args().collect();
    info!("{:?}", args);
    assert_eq!(args.len(), 3);

    let flags = &args[1];
    let dir = &args[2];

    assert_eq!(flags, "-c");

    let p = Path::new(dir);
    assert!(env::set_current_dir(&p).is_ok());

    let hostip = fs::read_to_string("cfg/HostIP")
        .unwrap_or(String::from("127.0.0.1"))
        .replace("\n", "");
    info!("{:?}", hostip);

    let listenaddr = format!("{}:0", hostip);
    info!("{:?}", listenaddr);

    let caller_sock = TcpListener::bind(listenaddr.clone()).unwrap();
    let callee_sock = TcpListener::bind(listenaddr).unwrap();

    //let listenport = caller_sock.local_addr().unwrap().port();
    //println!("{:?} {:?}", caller_sock, listenport);

    let caller_str = caller_sock.local_addr().unwrap().to_string();
    let callee_str = callee_sock.local_addr().unwrap().to_string();

    let len_caller = caller_str.len();
    let len_callee = callee_str.len();

    let faux_xferout = format!(
        "0901202{}{}02{}{}",
        xenc(len_caller),
        caller_str,
        xenc(len_callee),
        callee_str
    );
    info!("{:?}", faux_xferout);

    let mut xinetd_stream = unsafe { TcpStream::from_raw_fd(0) };
    info!("{:?}", xinetd_stream);

    let buf = faux_xferout.as_bytes();

    xinetd_stream.write_all(buf).unwrap();

    // wait up to 30 secondss for the first set of
    // control connections to get set up.
    reset(&startup_wd_tx, 30);

    let (caller_ctrl, _) = caller_sock.accept().unwrap();
    let (callee_ctrl, _) = callee_sock.accept().unwrap();

    let shovel_er = caller_ctrl.try_clone().expect("can't clone");
    let shovel_ee = callee_ctrl.try_clone().expect("can't clone");

    thread::spawn(|| {
        shovel(caller_ctrl, callee_ctrl, true);
    });
    thread::spawn(|| {
        shovel(shovel_ee, shovel_er, true);
    });

    // die after an hour, no matter what is going on
    // just to avoid people turning it on and walking away
    reset(&startup_wd_tx, 3600);

    // try this simpler technique, rather than accepting in arbitrary
    // order, and pairing them. instead, accept 1 from caller, then 1 from
    // callee, and pair those. this won't work quite as well if the
    // the connections don't come in pairs (like maybe one gets stuck
    // we would be stuck waiting for the next connect that might never come.)

    let (rende_tx, rende_cmd) = unbounded();
    thread::spawn(|| {
        watchdog(rende_cmd, 3600);
    });
    loop {
        reset(&rende_tx, 3600);
        let (caller_media, _) = caller_sock.accept().unwrap();
        info!("got 1 {:?}", caller_media);

        reset(&rende_tx, 10);
        let (callee_media, _) = callee_sock.accept().unwrap();
        info!("got 2 {:?}", callee_media);

        let shovel_er_media = caller_media.try_clone().expect("can't clone");
        let shovel_ee_media = callee_media.try_clone().expect("can't clone");

        thread::spawn(|| {
            shovel(caller_media, callee_media, false);
        });
        thread::spawn(|| {
            shovel(shovel_ee_media, shovel_er_media, false);
        });
    }
}
