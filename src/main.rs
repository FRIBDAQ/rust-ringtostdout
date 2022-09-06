//! # ringtostdout program
//!
//!  This program attaches as a consumer to a ringbuffer and
//! takes all the data that buffer has to offer and shoots it out
//! stdout.  
//!
//! ## Usage:
//!  
//!     ringtostdout option....
//! Where the options are:
//!
//! *   --directory - the directory with the ringbuffers.  If not
//! present on the command line defaults to /dev/shm (suitable for
//! linux)
//! *   --ring - name of the ring buffer file in that directory
//! we'll prepend the directory path.
//! *   --port - Portmanager (not ringmaster) listen port, if not present,
//! defaults to 30000 the standard.
//! *   --comment - If present, the supplied text is used to construct
//! a header visible in system displays of processes.  The ringmaster uses this
//! to indicate where the ringtostdout programs it spawns off will be sending
//! its data (ringmaster will have arranged for the stdout of ringtostdout to
//! be a socket to a client (which will get spawned off to be an stdintoring)).

pub mod ringmaster_client;
use clap::{App, Arg};
use std::fs;
use std::io;
use std::io::Write;
use std::path;
use std::process;
use std::time::Duration;

/// These are the program arguments processed by clap:
///
#[derive(Debug)]
struct ProgramArguments {
    directory: String,
    ring_name: String,
    portman: u16,
    comment: String,
}
// The implementation of the program arguments just provides a method
// to initialize one with the appropriate defaults.
impl ProgramArguments {
    /// Build a program arguments struct with the defaults described
    /// in the package's comments for argument defaults:
    ///
    fn new() -> ProgramArguments {
        ProgramArguments {
            directory: String::from("/dev/shm"),
            ring_name: String::from(""), // no default
            portman: 30000,
            comment: String::from(""),
        }
    }
}
fn main() {
    let args = process_args();
    eprintln!("{:#?}", args);

    // The next step in the game is to establish ourselves as a consumer of
    // the specified ring.  To do that we need to construct the full ringbuffer
    // path:

    let mut path_buf = path::PathBuf::from(args.directory);
    path_buf.push(args.ring_name);

    match ringmaster_client::attach_consumer(path_buf.to_str().expect("BUG")) {
        Err(e) => {
            eprintln!("Failed to attach ring buffer : {}", e);
            process::exit(-1);
        }
        Ok(consumer_info) => {
            match consumer_info.client {
                ringmaster_client::ClientType::Consumer(mut c) => {
                    output_data(&mut c);
                }
                ringmaster_client::ClientType::Producer(_p) => {
                    // This is a bad bug we're supposed to be a consumer:

                    eprintln!("ERROR - a producer was returned not a consumer");
                    process::exit(-1);
                }
            }
        }
    };
    process::exit(-1); // all exits are bad news:
}
//
// Main loop of the program.
// Each get, we try to do in MByte chunks which we then
// send to stdout.  We use timed_get with a timeout of a 1ms to reduce
// latency.
//
fn output_data(ring: &mut nscldaq_ringbuffer::ringbuffer::consumer::Consumer) {
    // We must use a vec -- or a static buffer else the buffer will 
    // overflow the stack.  Vec will allocate on the heap,
    // Note that evidently, the vector & can be treated as &[u8] which is
    // what both timed_get and write_all need.
    let mut data = Vec::<u8>::new();
    data.reserve(1024 * 1024);
    data.resize(1024 * 1024, 0);
    loop {
        match ring.timed_get(&mut data, Duration::from_millis(1)) {
            Ok(n) => {
                // Actually read n bytes.  We need to send them as binary
                // to stdout.

                io::stdout()
                    .write_all(&data[0..n])
                    .expect("Failed to write to stdout");
            }
            Err(e) => {
                // Time out is ok but anything else is fatal:

                match e {
                    nscldaq_ringbuffer::ringbuffer::consumer::Error::Timeout => {}
                    _ => {
                        eprintln!(
                            "Error reading from ring buffer: {}",
                            nscldaq_ringbuffer::ringbuffer::consumer::error_string(&e)
                        );
                        break;
                    }
                }
            }
        }
    }
}
// Define and process the arguments using clap (old since we need an older
// rust edition than current:

fn process_args() -> ProgramArguments {
    let mut result = ProgramArguments::new();

    // Use clap to define the options described in the program comments:
    // then run a parse on the argv:

    let parser = App::new(env!("CARGO_BIN_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author("Ron Fox")
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            Arg::with_name("directory")
                .short("d")
                .long("directory")
                .value_name("DIRECTORY")
                .help("Directory of ring buffer shared memory files managed by the ringmaster")
                .takes_value(true)
                .default_value("/dev/shm"),
        )
        .arg(
            Arg::with_name("ring_name")
                .short("r")
                .long("ring")
                .value_name("RINGBUFFER")
                .help("Name of the ring buffer we should take data from")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("portman")
                .short("p")
                .long("port")
                .value_name("PORTMAN_PORT")
                .help("TCP/IP port on which the port manager is listening")
                .takes_value(true)
                .default_value("30000"),
        )
        .arg(
            Arg::with_name("comment")
                .short("c")
                .long("comment")
                .value_name("COMMENT")
                .takes_value(true),
        )
        .get_matches();

    // override default directory - the diretory must exist:

    if let Some(directory) = parser.value_of("directory") {
        if fs::read_dir(directory).is_err() {
            eprintln!("{} Must be a readable directory", directory);
            process::exit(-1);
        } else {
            result.directory = String::from(directory);
        }
    }
    // ring name must be present else the program can't run:

    if let Some(ring) = parser.value_of("ring_name") {
        // We'll validate the ring when we attempt to map it:

        result.ring_name = String::from(ring);
    } else {
        eprintln!("The --ring option is required");
        process::exit(-1);
    }
    // Override the default port manager listen port.

    if let Some(port) = parser.value_of("portman") {
        // must parse as a u16:

        if let Ok(port_num) = port.parse::<u16>() {
            result.portman = port_num;
        } else {
            eprintln!("The port number {} must be an unsigned integer.", port);
            process::exit(-1);
        }
    }
    // If there's a comment set it - any sort of string is good:

    if let Some(comment) = parser.value_of("comment") {
        if comment != "" {
            result.comment = String::from(comment);
        }
    }
    result
}
