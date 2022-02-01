use std::env;
use std::path::Path;
use std::fs;
use std::net::TcpListener;
use std::net::TcpStream;
use std::io::Write;
use std::time::Duration;
use std::thread;
use std::io::BufRead;
use std::io::BufReader;
use std::process;
use std::net::Shutdown;
use log::info;
//use std::sync::mpsc::{SyncSender, Receiver};
//use std::sync::mpsc;


fn xenc(i: usize) -> String {
	let istr = i.to_string();
	let len = istr.len();
	format!("{:02}{}", len, istr)
}

fn
zzz() {

    thread::sleep(Duration::from_millis(4000))
}

fn
die() {
	println!("DIE");
}

fn shovel(inp: TcpStream, mut outp: TcpStream, die_hard: bool) {
	
	let dead_inp = inp.try_clone().expect("can't clone in shovel");
	let mut buf = BufReader::new(inp);
	
	loop {
		if die_hard {
			//death_vigil.notify();
		}
		// if this unwrap fails, does it zap the thread? or the
		// whole process?
		let bytes = buf.fill_buf().unwrap();
		let len_read = bytes.len();
		if len_read == 0 {
			if die_hard  {
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

	let args: Vec<String> = env::args().collect();
println!("{:?}", args);

	assert_eq!(args.len(), 3);

	let flags = &args[1];
	let dir = &args[2];

	assert_eq!(flags, "-c");

	let p = Path::new(dir);
	assert!(env::set_current_dir(&p).is_ok());

	let hostip = fs::read_to_string("cfg/HostIP").unwrap_or(String::from("127.0.0.1")).replace("\n", "");
	println!("{:?}", hostip);

	let listenaddr = format!("{}:0", hostip);
	println!("{:?}", listenaddr);

	let caller_sock = TcpListener::bind(listenaddr.clone()).unwrap();
	let callee_sock = TcpListener::bind(listenaddr).unwrap();

	//let listenport = caller_sock.local_addr().unwrap().port();
	//println!("{:?} {:?}", caller_sock, listenport);

	let caller_str = caller_sock.local_addr().unwrap().to_string();
	let callee_str = callee_sock.local_addr().unwrap().to_string();

	let len_caller = caller_str.len();
	let len_callee = callee_str.len();

	let faux_xferout = format!("0901202{}{}02{}{}", xenc(len_caller), caller_str, xenc(len_callee), callee_str);
println!("{:?}", faux_xferout);

/*
	let mut xinetd_stream  = unsafe { TcpStream::from_raw_fd(0) } ;
println!("{:?}", xinetd_stream);

	let buf = faux_xferout.as_bytes();

	xinetd_stream.write_all(buf).unwrap();
*/

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
	
/*
	let (txer, rxer): (SyncSender<TcpStream>, Receiver<TcpStream>) = mpsc::sync_channel(4);
	let (txee, rxee): (SyncSender<TcpStream>, Receiver<TcpStream>) = mpsc::sync_channel(4);

	thread::spawn(|| {
		rendevous_forever(caller_sock, txer);
	});
	thread::spawn(|| {
		rendevous_forever(callee_sock, txee);
	});
*/
	// die after an hour, not matter what is going on
	// just avoid people turning it on and walking away

	// try this simpler technique, rather than accepting in arbitrary
	// order, and pairing them. instead, accept 1 from caller, then 1 from
	// callee, and pair those. this won't work quite as well if the
	// the connections don't come in pairs (like maybe one gets stuck
	// we would be stuck waiting for the next connect that might never come.)

	loop {
		let (caller_media, _) = caller_sock.accept().unwrap();
info!("got 1 {:?}", caller_media);
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
