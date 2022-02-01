use std::env;
use std::path::Path;
use std::fs;
use std::net;
use std::net::TcpListener;
use std::net::TcpStream;
use std::os::unix::io::{FromRawFd, IntoRawFd, RawFd};
use std::io::Write;
use vigil::Vigil;
use std::time::Duration;
use std::thread;
use std::io::BufRead;
use std::io::BufReader;
use std::process;
use std::net::Shutdown;

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
	
	let (death_vigil, _) = Vigil::create(1200 * 1000, Some(Box::new(die)), None, None);
	let dead_inp = inp.try_clone().expect("can't clone in shovel");
	let mut buf = BufReader::new(inp);
	
	loop {
		if die_hard {
			death_vigil.notify();
		}
		// if this unwrap fails, does it zap the thread? or the
		// whole process?
		let bytes = buf.fill_buf().unwrap();
		let len_read = bytes.len();
		if len_read == 0 {
			if die_hard  {
				process::exit(0);
			}
		} else {
			dead_inp.shutdown(Shutdown::Both);
			outp.shutdown(Shutdown::Both);
			break;
		}
		outp.write_all(bytes).unwrap();
		buf.consume(len_read);

	}
}

fn main() {

	let (startup_vigil, _thread) = Vigil::create(1000, Some(Box::new(die)), None, Some(Box::new(die)));
	startup_vigil.notify();
	//zzz();

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

	startup_vigil.notify();

	startup_vigil.set_interval(10000);

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

	loop { zzz();}
}