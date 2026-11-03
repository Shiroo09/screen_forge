//! Minimal protobuf wire encoder. Port of `ScreenForge/Translate/ProtoWriter.cs`.
//!
//! Only what the Lens request needs: varints, length-delimited fields and nested
//! messages. The C# builder API used closures to fill a child writer; here a field
//! builder returns the child bytes, which keeps the call sites readable.

#[derive(Default)]
pub struct ProtoWriter {
    buffer: Vec<u8>,
}

impl ProtoWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buffer
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.buffer
    }

    pub fn write_varint(&mut self, mut value: u64) {
        while value >= 0x80 {
            self.buffer.push((value as u8) | 0x80);
            value >>= 7;
        }
        self.buffer.push(value as u8);
    }

    pub fn write_tag(&mut self, field_number: u32, wire_type: u32) {
        self.write_varint(((field_number << 3) | wire_type) as u64);
    }

    pub fn write_bytes(&mut self, field_number: u32, data: &[u8]) {
        self.write_tag(field_number, 2);
        self.write_varint(data.len() as u64);
        self.buffer.extend_from_slice(data);
    }

    pub fn write_string(&mut self, field_number: u32, value: &str) {
        self.write_bytes(field_number, value.as_bytes());
    }

    pub fn write_uint64(&mut self, field_number: u32, value: u64) {
        self.write_tag(field_number, 0);
        self.write_varint(value);
    }

    /// C# `WriteInt32` casts through `unchecked((ulong)value)`, so a negative
    /// value becomes the full 10-byte varint. `value as u64` matches that.
    pub fn write_int32(&mut self, field_number: u32, value: i32) {
        self.write_tag(field_number, 0);
        self.write_varint(value as u64);
    }

    pub fn write_enum(&mut self, field_number: u32, value: i32) {
        self.write_int32(field_number, value);
    }

    /// Nested message. `build` fills a child writer; this appends it as a field.
    pub fn write_message(&mut self, field_number: u32, build: impl FnOnce(&mut ProtoWriter)) {
        let mut inner = ProtoWriter::new();
        build(&mut inner);
        self.write_bytes(field_number, &inner.buffer);
    }
}
