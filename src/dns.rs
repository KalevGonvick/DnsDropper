use std::io::{Error, ErrorKind, Result};
use std::net::{Ipv4Addr, Ipv6Addr};

const PACKET_SIZE: usize = 4096;

// TODO - Modern DNS is no longer limited to 512 bytes in size, we should handle larger amounts.
pub struct BytePacketBuffer {
    pub buf: [u8; PACKET_SIZE],
    pub pos: usize,
}

impl BytePacketBuffer {
    pub fn new() -> BytePacketBuffer {
        BytePacketBuffer {
            buf: [0; PACKET_SIZE],
            pos: 0,
        }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    fn step(&mut self, steps: usize) -> Result<()> {
        self.pos += steps;

        Ok(())
    }

    fn seek(&mut self, pos: usize) -> Result<()> {
        self.pos = pos;

        Ok(())
    }

    fn read(&mut self) -> Result<u8> {
        if self.pos >= PACKET_SIZE {
            return Err(Error::new(ErrorKind::InvalidInput, "End of buffer"));
        }
        let res = self.buf[self.pos];
        self.pos += 1;

        Ok(res)
    }

    fn get(&mut self, pos: usize) -> Result<u8> {
        if pos >= PACKET_SIZE {
            return Err(Error::new(ErrorKind::InvalidInput, "End of buffer"));
        }
        Ok(self.buf[pos])
    }

    pub fn get_range(&mut self, start: usize, len: usize) -> Result<&[u8]> {
        if start + len >= PACKET_SIZE {
            return Err(Error::new(ErrorKind::InvalidInput, "End of buffer"));
        }
        Ok(&self.buf[start..start + len])
    }

    fn read_u16(&mut self) -> Result<u16> {
        let res = ((self.read()? as u16) << 8) | (self.read()? as u16);
        Ok(res)
    }

    fn read_u32(&mut self) -> Result<u32> {
        let res = ((self.read()? as u32) << 24)
            | ((self.read()? as u32) << 16)
            | ((self.read()? as u32) << 8)
            | ((self.read()? as u32) << 0);
        Ok(res)
    }

    fn read_qname(&mut self, outstr: &mut String) -> Result<()> {
        let mut pos = self.pos();
        let mut jumped = false;
        let mut delim = "";
        loop {
            let len = self.get(pos)?;

            // A two byte sequence, where the two highest bits of the first byte is
            // set, represents an offset relative to the start of the buffer. We
            // handle this by jumping to the offset, setting a flag to indicate
            // that we shouldn't update the shared buffer position once done.
            if (len & 0xC0) == 0xC0 {
                // When a jump is performed, we only modify the shared buffer
                // position once, and avoid making the change later on.
                if !jumped {
                    self.seek(pos + 2)?;
                }

                let b2 = self.get(pos + 1)? as u16;
                let offset = (((len as u16) ^ 0xC0) << 8) | b2;
                pos = offset as usize;
                jumped = true;
                continue;
            }

            pos += 1;

            // Names are terminated by an empty label of length 0
            if len == 0 {
                break;
            }

            outstr.push_str(delim);

            let str_buffer = self.get_range(pos, len as usize)?;
            outstr.push_str(&String::from_utf8_lossy(str_buffer).to_lowercase());
            delim = ".";
            pos += len as usize;
        }

        if !jumped {
            self.seek(pos)?;
        }

        Ok(())
    }

    fn write(&mut self, val: u8) -> Result<()> {
        if self.pos >= PACKET_SIZE {
            return Err(Error::new(ErrorKind::InvalidInput, "End of buffer"));
        }
        self.buf[self.pos] = val;
        self.pos += 1;
        Ok(())
    }

    fn write_u8(&mut self, val: u8) -> Result<()> {
        self.write(val)?;
        Ok(())
    }

    fn write_u16(&mut self, val: u16) -> Result<()> {
        self.write((val >> 8) as u8)?;
        self.write((val & 0xFF) as u8)?;
        Ok(())
    }

    fn write_u32(&mut self, val: u32) -> Result<()> {
        self.write((val >> 24) as u8)?;
        self.write((val >> 16) as u8)?;
        self.write((val >> 8) as u8)?;
        self.write((val >> 0) as u8)?;
        Ok(())
    }

    fn write_qname(&mut self, qname: &str) -> Result<()> {
        let split_str = qname.split('.').collect::<Vec<&str>>();
        for label in split_str {
            let len = label.len();
            if len > 0x34 {
                return Err(Error::new(ErrorKind::InvalidInput, "Single label exceeds 63 characters of length"));
            }
            self.write_u8(len as u8)?;
            for b in label.as_bytes() {
                self.write_u8(*b)?;
            }
        }
        self.write_u8(0)?;
        Ok(())
    }

    fn set(&mut self, pos: usize, val: u8) -> Result<()> {
        self.buf[pos] = val;
        Ok(())
    }

    fn set_u16(&mut self, pos: usize, val: u16) -> Result<()> {
        self.set(pos, (val >> 8) as u8)?;
        self.set(pos + 1, val as u8)?;
        Ok(())
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ResultCode {
    NOERROR = 0,
    FORMERR = 1,
    SERVFAIL = 2,
    NXDOMAIN = 3,
    NOTIMP = 4,
    REFUSED = 5,
}

impl ResultCode {
    pub fn from_num(num: u8) -> ResultCode {
        match num {
            1 => ResultCode::FORMERR,
            2 => ResultCode::SERVFAIL,
            3 => ResultCode::NXDOMAIN,
            4 => ResultCode::NOTIMP,
            5 => ResultCode::REFUSED,
            0 | _ => ResultCode::NOERROR,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DnsHeader {
    pub id: u16,
    // 16 bits
    pub recursion_desired: bool,
    // 1 bit
    pub truncated_message: bool,
    // 1 bit
    pub authoritative_answer: bool,
    // 1 bit
    pub opcode: u8,
    // 4 bits
    pub response: bool,
    // 1 bit
    pub result_code: ResultCode,
    // 4 bits
    pub checking_disabled: bool,
    // 1 bit
    pub authed_data: bool,
    // 1 bit
    pub z: bool,
    // 1 bit
    pub recursion_available: bool,
    // 1 bit
    pub questions: u16,
    // 16 bits
    pub answers: u16,
    // 16 bits
    pub authoritative_entries: u16,
    // 16 bits
    pub resource_entries: u16,      // 16 bits
}

impl DnsHeader {
    pub fn new() -> DnsHeader {
        DnsHeader {
            id: 0,

            recursion_desired: false,
            truncated_message: false,
            authoritative_answer: false,
            opcode: 0,
            response: false,

            result_code: ResultCode::NOERROR,
            checking_disabled: false,
            authed_data: false,
            z: false,
            recursion_available: false,

            questions: 0,
            answers: 0,
            authoritative_entries: 0,
            resource_entries: 0,
        }
    }

    pub fn read(&mut self, buffer: &mut BytePacketBuffer) -> Result<()> {
        self.id = buffer.read_u16()?;

        let flags = buffer.read_u16()?;
        let a = (flags >> 8) as u8;
        let b = (flags >> 0) as u8;
        self.recursion_desired = (a & (1 << 0)) > 0;
        self.truncated_message = (a & (1 << 1)) > 0;
        self.authoritative_answer = (a & (1 << 2)) > 0;
        self.opcode = (a >> 3) & 0x0F;
        self.response = (a & (1 << 7)) > 0;

        self.result_code = ResultCode::from_num(b & 0x0F);
        self.checking_disabled = (b & (1 << 4)) > 0;
        self.authed_data = (b & (1 << 5)) > 0;
        self.z = (b & (1 << 6)) > 0;
        self.recursion_available = (b & (1 << 7)) > 0;

        self.questions = buffer.read_u16()?;
        self.answers = buffer.read_u16()?;
        self.authoritative_entries = buffer.read_u16()?;
        self.resource_entries = buffer.read_u16()?;

        // Return the constant header size
        Ok(())
    }

    pub fn write(&self, buffer: &mut BytePacketBuffer) -> Result<()> {
        buffer.write_u16(self.id)?;

        (buffer.write_u8(
            ((self.recursion_desired as u8))
                | ((self.truncated_message as u8) << 1)
                | ((self.authoritative_answer as u8) << 2)
                | ((self.opcode) << 3)
                | ((self.response as u8) << 7),
        ))?;

        (buffer.write_u8(
            ((self.result_code.clone() as u8))
                | ((self.checking_disabled as u8) << 4)
                | ((self.authed_data as u8) << 5)
                | ((self.z as u8) << 6)
                | ((self.recursion_available as u8) << 7),
        ))?;

        buffer.write_u16(self.questions)?;
        buffer.write_u16(self.answers)?;
        buffer.write_u16(self.authoritative_entries)?;
        buffer.write_u16(self.resource_entries)?;

        Ok(())
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Hash, Copy)]
pub enum RecordType {
    UNKNOWN(u16),
    A,
    NS,
    CNAME,
    MX,
    AAAA,
    HTTPS,
}

impl RecordType {
    pub fn to_num(&self) -> u16 {
        match *self {
            RecordType::UNKNOWN(x) => x,
            RecordType::A => 1,
            RecordType::NS => 2,
            RecordType::CNAME => 5,
            RecordType::MX => 15,
            RecordType::AAAA => 28,
            RecordType::HTTPS => 65
        }
    }

    pub fn from_num(num: u16) -> RecordType {
        match num {
            1 => RecordType::A,
            2 => RecordType::NS,
            5 => RecordType::CNAME,
            15 => RecordType::MX,
            28 => RecordType::AAAA,
            65 => RecordType::HTTPS,
            _ => RecordType::UNKNOWN(num),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsQuestion {
    pub name: String,
    pub record_type: RecordType,
}

impl DnsQuestion {
    pub fn new(name: String, record_type: RecordType) -> DnsQuestion {
        DnsQuestion {
            name,
            record_type,
        }
    }

    pub fn read(&mut self, buffer: &mut BytePacketBuffer) -> Result<()> {
        buffer.read_qname(&mut self.name)?;
        self.record_type = RecordType::from_num(buffer.read_u16()?); // record_type
        let _ = buffer.read_u16()?; // class

        Ok(())
    }

    pub fn write(&self, buffer: &mut BytePacketBuffer) -> Result<()> {
        buffer.write_qname(&self.name)?;

        let type_as_num = self.record_type.to_num();
        buffer.write_u16(type_as_num)?;
        buffer.write_u16(1)?;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[allow(dead_code)]
pub enum DnsRecord {
    /* 0 */
    UNKNOWN {
        domain: String,
        record_type: u16,
        data_len: u16,
        ttl: u32,
    },

    /* 1 */
    A {
        domain: String,
        addr: Ipv4Addr,
        ttl: u32,
    },

    /* 2 */
    NS {
        domain: String,
        host: String,
        ttl: u32,
    },

    /* 5 */
    CNAME {
        domain: String,
        host: String,
        ttl: u32,
    },

    /* 15 */
    MX {
        domain: String,
        priority: u16,
        host: String,
        ttl: u32,
    },

    /* 28 */
    AAAA {
        domain: String,
        addr: Ipv6Addr,
        ttl: u32,
    },
}

impl DnsRecord {
    pub fn read(buffer: &mut BytePacketBuffer) -> Result<DnsRecord> {
        let mut domain = String::new();
        buffer.read_qname(&mut domain)?;

        let record_type_num = buffer.read_u16()?;
        let record_type = RecordType::from_num(record_type_num);
        let _ = buffer.read_u16()?;
        let ttl = buffer.read_u32()?;
        let data_len = buffer.read_u16()?;

        match record_type {
            RecordType::A => {
                let raw_addr = buffer.read_u32()?;
                let addr = Ipv4Addr::new(
                    (raw_addr >> 24) as u8,
                    (raw_addr >> 16) as u8,
                    (raw_addr >> 8) as u8,
                    (raw_addr) as u8,
                );

                Ok(DnsRecord::A { domain, addr, ttl })
            }
            RecordType::AAAA => {
                let raw_addr1 = buffer.read_u32()?;
                let raw_addr2 = buffer.read_u32()?;
                let raw_addr3 = buffer.read_u32()?;
                let raw_addr4 = buffer.read_u32()?;
                let addr = Ipv6Addr::new(
                    (raw_addr1 >> 16) as u16,
                    (raw_addr1) as u16,
                    (raw_addr2 >> 16) as u16,
                    (raw_addr2) as u16,
                    (raw_addr3 >> 16) as u16,
                    (raw_addr3) as u16,
                    (raw_addr4 >> 16) as u16,
                    (raw_addr4) as u16,
                );

                Ok(DnsRecord::AAAA {
                    domain,
                    addr,
                    ttl,
                })
            }
            RecordType::NS => {
                let mut ns = String::new();
                buffer.read_qname(&mut ns)?;

                Ok(DnsRecord::NS {
                    domain,
                    host: ns,
                    ttl,
                })
            }
            RecordType::CNAME => {
                let mut cname = String::new();
                buffer.read_qname(&mut cname)?;

                Ok(DnsRecord::CNAME {
                    domain,
                    host: cname,
                    ttl,
                })
            }
            RecordType::MX => {
                let priority = buffer.read_u16()?;
                let mut mx = String::new();
                buffer.read_qname(&mut mx)?;

                Ok(DnsRecord::MX {
                    domain,
                    priority,
                    host: mx,
                    ttl,
                })
            }
            RecordType::UNKNOWN(_) => {
                buffer.step(data_len as usize)?;

                Ok(DnsRecord::UNKNOWN {
                    domain,
                    record_type: record_type_num,
                    data_len,
                    ttl,
                })
            }
            RecordType::HTTPS => {
                buffer.step(data_len as usize)?;

                Ok(DnsRecord::UNKNOWN {
                    domain,
                    record_type: record_type_num,
                    data_len,
                    ttl,
                })
            }
        }
    }

    pub fn write(&self, buffer: &mut BytePacketBuffer) -> Result<usize> {
        let start_pos = buffer.pos();

        match *self {
            DnsRecord::A {
                ref domain,
                ref addr,
                ttl,
            } => {
                buffer.write_qname(domain)?;
                buffer.write_u16(RecordType::A.to_num())?;
                buffer.write_u16(1)?;
                buffer.write_u32(ttl)?;
                buffer.write_u16(4)?;

                let octets = addr.octets();
                buffer.write_u8(octets[0])?;
                buffer.write_u8(octets[1])?;
                buffer.write_u8(octets[2])?;
                buffer.write_u8(octets[3])?;
            }
            DnsRecord::NS {
                ref domain,
                ref host,
                ttl,
            } => {
                buffer.write_qname(domain)?;
                buffer.write_u16(RecordType::NS.to_num())?;
                buffer.write_u16(1)?;
                buffer.write_u32(ttl)?;

                let pos = buffer.pos();
                buffer.write_u16(0)?;

                buffer.write_qname(host)?;

                let size = buffer.pos() - (pos + 2);
                buffer.set_u16(pos, size as u16)?;
            }
            DnsRecord::CNAME {
                ref domain,
                ref host,
                ttl,
            } => {
                buffer.write_qname(domain)?;
                buffer.write_u16(RecordType::CNAME.to_num())?;
                buffer.write_u16(1)?;
                buffer.write_u32(ttl)?;

                let pos = buffer.pos();
                buffer.write_u16(0)?;

                buffer.write_qname(host)?;

                let size = buffer.pos() - (pos + 2);
                buffer.set_u16(pos, size as u16)?;
            }
            DnsRecord::MX {
                ref domain,
                priority,
                ref host,
                ttl,
            } => {
                buffer.write_qname(domain)?;
                buffer.write_u16(RecordType::MX.to_num())?;
                buffer.write_u16(1)?;
                buffer.write_u32(ttl)?;

                let pos = buffer.pos();
                buffer.write_u16(0)?;

                buffer.write_u16(priority)?;
                buffer.write_qname(host)?;

                let size = buffer.pos() - (pos + 2);
                buffer.set_u16(pos, size as u16)?;
            }
            DnsRecord::AAAA {
                ref domain,
                ref addr,
                ttl,
            } => {
                buffer.write_qname(domain)?;
                buffer.write_u16(RecordType::AAAA.to_num())?;
                buffer.write_u16(1)?;
                buffer.write_u32(ttl)?;
                buffer.write_u16(16)?;

                for octet in &addr.segments() {
                    buffer.write_u16(*octet)?;
                }
            }
            DnsRecord::UNKNOWN { .. } => {
                logs::warn!("Skipping record: {:?}", self);
            }
        }

        Ok(buffer.pos() - start_pos)
    }
}

#[derive(Clone, Debug)]
pub struct DnsPacket {
    pub header: DnsHeader,
    pub questions: Vec<DnsQuestion>,
    pub answers: Vec<DnsRecord>,
    pub authorities: Vec<DnsRecord>,
    pub resources: Vec<DnsRecord>,
}

impl DnsPacket {
    pub fn new() -> DnsPacket {
        DnsPacket {
            header: DnsHeader::new(),
            questions: Vec::new(),
            answers: Vec::new(),
            authorities: Vec::new(),
            resources: Vec::new(),
        }
    }

    pub fn from_buffer(buffer: &mut BytePacketBuffer) -> Result<DnsPacket> {
        let mut result = DnsPacket::new();
        result.header.read(buffer)?;

        for _ in 0..result.header.questions {
            let mut question = DnsQuestion::new("".to_string(), RecordType::UNKNOWN(0));
            question.read(buffer)?;
            result.questions.push(question);
        }

        for _ in 0..result.header.answers {
            let rec = DnsRecord::read(buffer)?;
            result.answers.push(rec);
        }
        for _ in 0..result.header.authoritative_entries {
            let rec = DnsRecord::read(buffer)?;
            result.authorities.push(rec);
        }
        for _ in 0..result.header.resource_entries {
            let rec = DnsRecord::read(buffer)?;
            result.resources.push(rec);
        }

        Ok(result)
    }


    pub fn write(&mut self, buffer: &mut BytePacketBuffer) -> Result<()> {
        self.header.questions = self.questions.len() as u16;
        self.header.answers = self.answers.len() as u16;
        self.header.authoritative_entries = self.authorities.len() as u16;
        self.header.resource_entries = self.resources.len() as u16;

        self.header.write(buffer)?;

        for question in &self.questions {
            question.write(buffer)?;
        }
        for rec in &self.answers {
            rec.write(buffer)?;
        }
        for rec in &self.authorities {
            rec.write(buffer)?;
        }
        for rec in &self.resources {
            rec.write(buffer)?;
        }

        Ok(())
    }

    // TODO - these will be used for caching stuff later
    // pub fn get_random_a(&self) -> Option<String> {
    //     if !self.answers.is_empty() {
    //         let a_record = &self.answers[0];
    //         if let DnsRecord::A { ref addr, .. } = *a_record {
    //             return Some(addr.to_string());
    //         }
    //     }
    //     None
    // }

    // pub fn get_resolved_ns(&self, qname: &str) -> Option<String> {
    //     let mut new_authorities = Vec::new();
    //     for auth in &self.authorities {
    //         if let DnsRecord::NS {
    //             ref domain,
    //             ref host,
    //             ..
    //         } = *auth
    //         {
    //             if !qname.ends_with(domain) {
    //                 continue;
    //             }
    //
    //             for rsrc in &self.resources {
    //                 if let DnsRecord::A {
    //                     ref domain,
    //                     ref addr,
    //                     ttl,
    //                 } = *rsrc
    //                 {
    //                     if domain != host {
    //                         continue;
    //                     }
    //
    //                     let rec = DnsRecord::A {
    //                         domain: host.clone(),
    //                         addr: *addr,
    //                         ttl,
    //                     };
    //
    //                     new_authorities.push(rec);
    //                 }
    //             }
    //         }
    //     }
    //
    //     if !new_authorities.is_empty() {
    //         if let DnsRecord::A { addr, .. } = new_authorities[0] {
    //             return Some(addr.to_string());
    //         }
    //     }
    //
    //     None
    // }

    // pub fn get_unresolved_ns(&self, qname: &str) -> Option<String> {
    //     let mut new_authorities = Vec::new();
    //     for auth in &self.authorities {
    //         if let DnsRecord::NS {
    //             ref domain,
    //             ref host, ..
    //         } = *auth {
    //             if !qname.ends_with(domain) {
    //                 continue;
    //             }
    //             new_authorities.push(host);
    //         }
    //     }
    //
    //     if !new_authorities.is_empty() {
    //         return Some(new_authorities[0].clone());
    //     }
    //
    //     None
    // }
}
