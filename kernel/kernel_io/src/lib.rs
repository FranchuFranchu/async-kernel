#![cfg_attr(not(std), no_std)]
#![feature(associated_type_defaults, generic_associated_types)]

extern crate alloc;

use alloc::{boxed::Box, string::String, vec::Vec};

use async_trait::async_trait;
use kernel_syscall_abi::filesystem::{IoError as Error, IoErrorKind as ErrorKind};
pub type Result<T> = core::result::Result<T, Error>;

#[async_trait]
pub trait Read {
    type Error: Send + Sync;
    async fn read(&mut self, buf: &mut [u8]) -> core::result::Result<usize, Self::Error>;
    async fn read_vectored(
        &mut self,
        bufs: &mut [&mut [u8]],
    ) -> core::result::Result<usize, Self::Error> {
        let mut read = 0;
        for i in bufs {
            read += self.read(i).await?;
        }
        Ok(read)
    }
    async fn read_to_end(&mut self, buf: &mut Vec<u8>) -> core::result::Result<usize, Self::Error> {
        let mut read = 0;
        loop {
            buf.resize(read + 512, 0);
            let mut buf_here = &mut buf[read..];
            let read_here = self.read(buf_here).await?;
            read += read_here;
            if read_here == 0 {
                buf.resize(read, 0);
                return Ok(read);
            }
        }
    }
    async fn read_to_end_new(&mut self) -> core::result::Result<(usize, Vec<u8>), Self::Error> {
        let mut v = alloc::vec::Vec::new();
        Ok((self.read_to_end(&mut v).await?, v))
    }
    async fn read_to_string_new(
        &mut self,
    ) -> Result<core::result::Result<(usize, String), Self::Error>> {
        let mut v = alloc::string::String::new();
        let result = self.read_to_string(&mut v).await?;
        let result = match result {
            Ok(e) => e,
            Err(e) => return Ok(Err(e)),
        };
        Ok(Ok((result, v)))
    }
    async fn read_to_end_exact(
        &mut self,
        len: usize,
    ) -> Result<core::result::Result<Vec<u8>, Self::Error>> {
        let mut v = alloc::vec::Vec::new();
        v.resize(len, 0);
        let result = self.read_exact(&mut v).await?;
        let _result = match result {
            Ok(e) => e,
            Err(e) => return Ok(Err(e)),
        };
        Ok(Ok(v))
    }
    async fn read_to_string(
        &mut self,
        buf: &mut String,
    ) -> Result<core::result::Result<usize, Self::Error>> {
        let mut vec = alloc::vec::Vec::new();
        let read = self.read_to_end(&mut vec).await;
        let read = match read {
            Ok(e) => e,
            Err(e) => return Ok(Err(e)),
        };
        buf.insert_str(buf.len(), core::str::from_utf8(&vec)?);
        Ok(Ok(read))
    }
    async fn read_exact(
        &mut self,
        mut buf: &mut [u8],
    ) -> Result<core::result::Result<(), Self::Error>> {
        while !buf.is_empty() {
            match self.read(buf).await {
                Ok(0) => break,
                Ok(n) => {
                    let tmp = buf;
                    buf = &mut tmp[n..];
                }
                Err(e) => {
                    return Ok(Err(e));
                }
            }
        }
        if !buf.is_empty() {
            Err(Error::new_simple(ErrorKind::UnexpectedEof))
        } else {
            Ok(Ok(()))
        }
    }
}

#[async_trait]
pub trait Write {
    type Error: Send + Sync;

    async fn write(&mut self, buf: &[u8]) -> core::result::Result<usize, Self::Error>;
}
