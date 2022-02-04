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
        } else if new_ticks == -1 {
            info!("stop");
            break;
        } else {
            ticks_left = new_ticks;
            info!("reset {}", new_ticks);
        }
    }
}

fn reset(sender: &Sender<i32>, new_ticks: i32) {
    sender.try_send(new_ticks).unwrap();
}

fn xenc(i: usize) -> String {
    let istr = i.to_string();
    let len = istr.len();
    format!("{:02}{}", len, istr)
}

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
            reset(&shovel_wd_tx, 10 * 60);
        }
        // if this unwrap fails, does it zap the thread? or the
        // whole process?
        let bytes = buf.fill_buf().unwrap();
        let len_read = bytes.len();
        if len_read == 0 {
            if die_hard {
                process::exit(0);
            } else {
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

    // die after an hour, not matter what is going on
    // just avoid people turning it on and walking away
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

    //loop { zzz();}
}
