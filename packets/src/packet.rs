use std::io::{Error, ErrorKind};

pub mod dns;

pub struct BytePacketBuffer {
    pub buf: Vec<u8>,
    pub pos: usize,
    pub size: usize
}

impl BytePacketBuffer {
    pub fn new(packet_size: usize) -> BytePacketBuffer {
        BytePacketBuffer {
            buf: vec![0; packet_size],
            pos: 0,
            size: packet_size
        }
    }

    fn write(&mut self, val: u8) -> std::io::Result<()> {
        if self.pos >= self.size {
            return Err(Error::new(ErrorKind::InvalidInput, "End of buffer"));
        }
        self.buf[self.pos] = val;
        self.pos += 1;
        Ok(())
    }

    fn set(&mut self, pos: usize, val: u8) {
        self.buf[pos] = val;
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    fn read(&mut self) -> std::io::Result<u8> {

        if self.pos >= self.size {
            return Err(Error::new(ErrorKind::InvalidInput, "End of buffer"));
        }

        let res = self.buf[self.pos];
        self.pos += 1;
        Ok(res)
    }

    pub fn get(&mut self, pos: usize) -> std::io::Result<u8> {

        if pos >= self.size {
            return Err(Error::new(ErrorKind::InvalidInput, "End of buffer"));
        }

        Ok(self.buf[pos])
    }

    pub fn get_range(&mut self, start: usize, len: usize) -> std::io::Result<&[u8]> {

        if start + len >= self.size {
            return Err(Error::new(ErrorKind::InvalidInput, "End of buffer"));
        }

        Ok(&self.buf[start..start + len])
    }

    pub fn read_u16(&mut self) -> std::io::Result<u16> {
        let res = ((self.read()? as u16) << 8) | (self.read()? as u16);
        Ok(res)
    }

    pub fn read_u32(&mut self) -> std::io::Result<u32> {
        let res = ((self.read()? as u32) << 24)
            | ((self.read()? as u32) << 16)
            | ((self.read()? as u32) << 8)
            | ((self.read()? as u32) << 0);
        Ok(res)
    }

    pub(crate) fn step(&mut self, steps: usize) {
        self.pos += steps;
    }

    pub(crate) fn seek(&mut self, pos: usize) {
        self.pos = pos;
    }

    pub(crate) fn write_u8(&mut self, val: u8) -> std::io::Result<()> {
        self.write(val)?;
        Ok(())
    }

    pub(crate) fn write_u16(&mut self, val: u16) -> std::io::Result<()> {
        self.write((val >> 8) as u8)?;
        self.write((val & 0xFF) as u8)?;
        Ok(())
    }

    pub(crate) fn write_u32(&mut self, val: u32) -> std::io::Result<()> {
        self.write((val >> 24) as u8)?;
        self.write((val >> 16) as u8)?;
        self.write((val >> 8) as u8)?;
        self.write((val >> 0) as u8)?;
        Ok(())
    }


    pub(crate) fn set_u16(&mut self, pos: usize, val: u16) -> std::io::Result<()> {
        self.set(pos, (val >> 8) as u8);
        self.set(pos + 1, val as u8);
        Ok(())
    }
}