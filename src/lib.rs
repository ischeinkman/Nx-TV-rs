extern crate sdl2;
extern crate image;
extern crate libc;

use std::result::Result;
use std::path::Path;
use std::fs::File;
use std::fs::OpenOptions;
use std::error::Error;
use std::io::Write;
use std::io;
use std::os::unix::io::AsRawFd;
use std::panic;

mod decoder_shim;
use decoder_shim::{VideoDecoder, Codec, Frame};
mod rawbindings;
mod player;

use rawbindings::decoder::RawFfmpegDecoder;
use player::{play_video};


pub fn main() {
    redirect_stderr("nxtv_err.txt");
    redirect_stdout("nxtv_out.txt");
    match RawFfmpegDecoder::new("test.mp4") {
        Ok(codec) => {
            //eprintln!("Got codec {} for file {}.", codec.codec().name, codec.source());
            play_video(codec)
        },
        Err(e) => {
            eprintln!("Got error: {}", e);
            return
        }
    }
}

pub fn redirect_stdout (filename : &str) -> Result<File, io::Error> {
    let mut outfile = OpenOptions::new()
        .write(true)
        .create(true)
        .open(filename)?;
    outfile.write_fmt(format_args!("Redirecting standard output to {}.", filename))?;
    let raw_fd = outfile.as_raw_fd();
    let new_fd = unsafe {
        libc::fflush(0 as *mut libc::FILE);
        libc::dup2(raw_fd, libc::STDOUT_FILENO)
    };
    if new_fd != libc::STDOUT_FILENO {
        Err(io::Error::new(io::ErrorKind::Other, format!("Could not call dup2. Ended up redirecting fd {} to {} instead of {}.", raw_fd, new_fd, libc::STDOUT_FILENO)))
    }
    else { 
        Ok(outfile) 
    }
}

pub fn redirect_stderr (filename : &str) -> Result<File, io::Error> {
    let mut outfile = OpenOptions::new()
        .write(true)
        .create(true)
        .open(filename)?;
    outfile.write_fmt(format_args!("Redirecting standard error to {}.\n", filename))?;
    let raw_fd = outfile.as_raw_fd();
    let new_fd = unsafe {
        libc::fflush(0 as *mut libc::FILE);
        libc::dup2(raw_fd, libc::STDERR_FILENO)
    };
    if new_fd != libc::STDERR_FILENO {
        Err(io::Error::new(io::ErrorKind::Other, format!("Could not call dup2. Ended up redirecting fd {} to {} instead of {}.", raw_fd, new_fd, libc::STDERR_FILENO)))
    }
    else { 
        Ok(outfile) 
    }
}