
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
//! 


use std::net::TcpStream;             // In order to talk with ring master.
