//!
//! The ringmaster_client::client module contains the
//! code needed to interface with the ringmaster as a client
//! in this module, we confine ourselves to the REGISTER
//! operation as we need that to register a client
//! of the ringbuffer.
//! We'll provide code to create a client (producer or consumer)
//! that's been registered with the ringmaster and ready to go.
//! Note that it is the caller's responsibility to maintain
//! a connection to the ring master as long as it needs to be a
//! client.
//!
//! As such, this provides only support for clients to local rings.
//! We would need to implement the REMOTE request to support remote
//! ring buffer access.

use nscldaq_ringbuffer::ringbuffer::{consumer, producer, RingBufferMap};
use portman_client;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path;
use std::process;
use std::sync::{Arc, Mutex};

//
// Types of errors we can produce:
//
pub enum Error {
    ConsumerError(consumer::Error),
    ProducerError(producer::Error),
    MapError(String),
    PortManError(portman_client::Error),
    NoRingMaster,
    RingMasterFail(String),
    Unimplemented,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let stringified = match self {
            Error::ConsumerError(e) => {
                format!("Consumer error {}", consumer::error_string(e))
            }
            Error::ProducerError(e) => {
                format!("Producer error {}", producer::error_string(e))
            }
            Error::MapError(s) => format!("Ring Buffer Mapping error {}", s),
            Error::PortManError(e) => {
                format!("Error interacting with port manager: {}", e.to_string())
            }
            Error::NoRingMaster => format!("The ring master is not running"),
            Error::RingMasterFail(s) => format!("Interaction with ringmaster failed: {}", s),
            Error::Unimplemented => String::from("Unimplemented operation attempted"),
        };
        write!(f, "{}", stringified)
    }
}
//
// Types of clients:
//
pub enum ClientType {
    Consumer(consumer::Consumer),
    Producer(producer::Producer),
}
//
// Struct to hold what we need to maintain a connection to the
// ring master and to operate as a client:
/// The purpose of the ring_master field is just to
/// allow the socket connection to stay in scope.
///
#[allow(dead_code)]
#[allow(unused_variables)]
pub struct RingClient {
    pub client: ClientType,
    ring_master: TcpStream,
}

#[allow(non_upper_case_globals)]
static mut portman_port: u16 = 30000;

///
/// When we return a result, this is the type we return:
///
pub type RingClientResult = Result<RingClient, Error>;

///
/// Override the port manager default port for future
/// port manager operations:
///
pub fn set_portman_port(new_port: u16) {
    unsafe { portman_port = new_port };
}

/// Create a consumer of ring data.
/// This:
///
/// *  attach a ring buffer client to the ring.
/// *  contact the port manager to get the ring master port number.
/// *  send the appropriate CONNECT message to the ring master.
///
/// On success we're going to return a struct that consists of,
/// in order, the Consumer object we created and the TcpStream
/// that's holding the connection to the ring master.
///
pub fn attach_consumer(ring_buffer_file: &str) -> RingClientResult {
    println!("Attach_consumer {}", ring_buffer_file);
    match get_ringmaster_port() {
        Ok(port) => match RingBufferMap::new(ring_buffer_file) {
            Ok(raw_map) => {
                println!("Ring master is on {}", port);
                println!("Mapped the ring buffer");
                let safe_map = Arc::new(Mutex::new(raw_map));
                println!("made the safe map");
                match consumer::Consumer::attach(&Arc::clone(&safe_map)) {
                    Ok(consumer) => {
                        println!("Attached the ring as a consumer");
                        let slot = consumer.get_index();
                        println!("On slot {}", slot);
                        match connect_consumer(port, &ring_name(&ring_buffer_file), slot) {
                            Err(e) => Err(e),
                            Ok(stream) => Ok(RingClient {
                                client: ClientType::Consumer(consumer),
                                ring_master: stream,
                            }),
                        }
                    }
                    Err(e) => Err(Error::ConsumerError(e)),
                }
            }
            Err(s) => Err(Error::MapError(s)),
        },
        Err(e) => Err(e),
    }
}
///
/// Create a producer of data into a ringbuffer.
/// This:
///
/// *    Attaches a ringbuffer producer to the ring if possible.
/// *    Contacts the port manager to get the RingMaster port.
/// *    Sends the appropriate CONNECT message to the ring master
/// to nail down the producer slot.
///
/// On success we return a struct that contains the producer object
/// and the TCP/IP stream the application must keep open on the
/// ring master.   
///
///  
pub fn attach_producer(ring_buffer_file: &str) -> RingClientResult {
    // Note the logic is very similar to that of the attach_consumer.
    // with a bit of thought I could perhaps do some
    // factorization.

    match get_ringmaster_port() {
        Ok(port) => match RingBufferMap::new(ring_buffer_file) {
            Ok(raw_map) => {
                let safe_map = Arc::new(Mutex::new(raw_map));
                match producer::Producer::attach(&Arc::clone(&safe_map)) {
                    Ok(producer) => match connect_producer(port, &ring_name(&ring_buffer_file)) {
                        Err(e) => Err(e),
                        Ok(stream) => Ok(RingClient {
                            client: ClientType::Producer(producer),
                            ring_master: stream,
                        }),
                    },
                    Err(e) => Err(Error::ProducerError(e)),
                }
            }
            Err(s) => Err(Error::MapError(s)),
        },
        Err(e) => Err(e),
    }
}
/*-----------------------------------------------------------------
    Private functions.
    These functions are not exported to the clients of this
    module, but are convenience functions.

*/

// Return the port the ringmaster is listening on:
//
fn get_ringmaster_port() -> Result<u16, Error> {
    let port = unsafe { portman_port };
    let mut client = portman_client::Client::new(port);

    match client.find_by_service("RingMaster") {
        Err(e) => Err(Error::PortManError(e)),
        Ok(v) => {
            if v.len() == 0 {
                Err(Error::NoRingMaster)
            } else {
                Ok(v[0].port) // If there are several ports, return the first.
            }
        }
    }
}
//
// Take a full path to a ring buffer file and return just the filename (ring name)
// as that's what the ringmaster needs to see.
//
fn ring_name(filename: &str) -> String {
    String::from(
        path::Path::new(filename)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap(),
    )
}
//
// Tell the ring master we're connecting a consumer.
// This formats the CONNECT message, uses ringmaster_request
// for the rest of it.
//
fn connect_consumer(port: u16, ring: &str, slot: u32) -> Result<TcpStream, Error> {
    let request = format!("CONNECT {} consumer.{} {}\n", ring, slot, process::id());
    ringmaster_request(port, &request)
}
// Tell the ring master we're connecting a producer.
// Formats the message and lets ringmaster_request do the rest:
//
fn connect_producer(port: u16, ring: &str) -> Result<TcpStream, Error> {
    let request = format!("CONNECT {} producer {}", ring, process::id());

    ringmaster_request(port, &request)
}

// Does a ring master request and analyzes the result.

fn ringmaster_request(port: u16, request: &str) -> Result<TcpStream, Error> {
    println!("Ring master request '{}'", request);
    match TcpStream::connect(format!("127.0.0.1:{}", port).as_str()) {
        Err(_) => Err(Error::NoRingMaster),
        Ok(mut stream) => {
            println!("Stream connected");
            // write the request and use a buffered reader to get the reply line.
            // we can do this since while we need to keep the stream open we're not
            // interacting any more.

            if let Err(_) = stream.write_all(request.as_bytes()) {
                Err(Error::NoRingMaster)
            } else {
                if let Err(_) = stream.flush() {
                    Err(Error::NoRingMaster)
                } else {
                    println!("Request written and flushed");
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut line = String::new();
                    if let Ok(_n) = reader.read_line(&mut line) {
                        println!("Line: '{}'", line);

                        if line.trim() == "OK" {
                            Ok(stream)
                        } else {
                            Err(Error::RingMasterFail(line))
                        }
                    } else {
                        Err(Error::NoRingMaster)
                    }
                }
            }
        }
    }
}
