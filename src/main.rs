use std::env;
use std::path::Path;
use std::fs;
use std::net;
use std::net::TcpListener;
use std::net::TcpStream;
use std::os::unix::io::{FromRawFd, IntoRawFd, RawFd};
use std::io::Write;

fn xenc(i: usize) -> String {
	let istr = i.to_string();
	let len = istr.len();
	format!("{:02}{}", len, istr)
}

fn main() {

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

	let mut xinetd_stream  = unsafe { TcpStream::from_raw_fd(0) } ;
println!("{:?}", xinetd_stream);

	let buf = faux_xferout.as_bytes();

	xinetd_stream.write_all(buf).unwrap();

}
