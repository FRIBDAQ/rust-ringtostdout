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
