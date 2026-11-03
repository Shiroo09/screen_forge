//! GIF LZW compressor. Port of `ScreenForge/Gif/Encoder/LzwEncoder.cs`
//! (ScreenToGif -> Jef Poskanzer's Java adaptation).
//!
//! Supports a variable initial code size: a 64-color palette is written with
//! 6-bit codes instead of 8, which shrinks the output noticeably.

use std::io::Write;

const EOF: i32 = -1;
const MAX_BITS: i32 = 12;
const HASH_SIZE: usize = 5003;
const MAX_MAX_CODE: i32 = 1 << MAX_BITS;

const MASKS: [i32; 17] = [
    0x0000, 0x0001, 0x0003, 0x0007, 0x000F, 0x001F, 0x003F, 0x007F, 0x00FF, 0x01FF, 0x03FF, 0x07FF,
    0x0FFF, 0x1FFF, 0x3FFF, 0x7FFF, 0xFFFF,
];

pub struct LzwEncoder<'a> {
    pixels: &'a [u8],
    initial_code_size: i32,
    hash_table: [i32; HASH_SIZE],
    code_table: [i32; HASH_SIZE],
    accumulator: [u8; 256],

    current_pixel: usize,
    num_bits: i32,
    max_code: i32,
    free_entry: i32,
    clear_flag: bool,
    initial_bits: i32,
    clear_code: i32,
    eof_code: i32,
    bit_accumulator: i32,
    bit_count: i32,
    char_count: usize,
}

impl<'a> LzwEncoder<'a> {
    /// `indexed_pixels` are palette indices, one byte per pixel;
    /// `color_depth` is the bit count the palette needs (1-8).
    pub fn new(indexed_pixels: &'a [u8], color_depth: i32) -> Self {
        Self {
            pixels: indexed_pixels,
            // GIF requires a minimum LZW code size of 2.
            initial_code_size: color_depth.clamp(2, 8),
            hash_table: [0; HASH_SIZE],
            code_table: [0; HASH_SIZE],
            accumulator: [0; 256],
            current_pixel: 0,
            num_bits: 0,
            max_code: 0,
            free_entry: 0,
            clear_flag: false,
            initial_bits: 0,
            clear_code: 0,
            eof_code: 0,
            bit_accumulator: 0,
            bit_count: 0,
            char_count: 0,
        }
    }

    pub fn encode<W: Write>(&mut self, output: &mut W) {
        let _ = output.write_all(&[self.initial_code_size as u8]);
        self.current_pixel = 0;
        self.compress(self.initial_code_size + 1, output);
        let _ = output.write_all(&[0]); // block terminator
    }

    fn compress<W: Write>(&mut self, initial_bits: i32, output: &mut W) {
        self.initial_bits = initial_bits;
        self.clear_flag = false;
        self.num_bits = initial_bits;
        self.max_code = max_code(self.num_bits);
        self.clear_code = 1 << (initial_bits - 1);
        self.eof_code = self.clear_code + 1;
        self.free_entry = self.clear_code + 2;
        self.char_count = 0;
        self.bit_accumulator = 0;
        self.bit_count = 0;

        let mut entry = self.next_pixel();
        if entry == EOF {
            self.output(self.clear_code, output);
            self.output(self.eof_code, output);
            return;
        }

        let mut hash_shift = 0;
        let mut code = HASH_SIZE;
        while code < 65536 {
            hash_shift += 1;
            code *= 2;
        }
        hash_shift = 8 - hash_shift;

        self.reset_hash_table();
        self.output(self.clear_code, output);

        loop {
            let c = self.next_pixel();
            if c == EOF {
                break;
            }

            let fcode = (c << MAX_BITS) + entry;
            let mut i = ((c << hash_shift) ^ entry) as usize;

            if self.hash_table[i] == fcode {
                entry = self.code_table[i];
                continue;
            }

            if self.hash_table[i] >= 0 {
                let disp = if i == 0 { 1 } else { HASH_SIZE - i };
                let mut found = false;
                loop {
                    let next = i as i32 - disp as i32;
                    i = if next < 0 {
                        (next + HASH_SIZE as i32) as usize
                    } else {
                        next as usize
                    };
                    if self.hash_table[i] == fcode {
                        entry = self.code_table[i];
                        found = true;
                        break;
                    }
                    if self.hash_table[i] < 0 {
                        break;
                    }
                }
                if found {
                    continue;
                }
            }

            self.output(entry, output);
            entry = c;

            if self.free_entry < MAX_MAX_CODE {
                self.code_table[i] = self.free_entry;
                self.free_entry += 1;
                self.hash_table[i] = fcode;
            } else {
                self.reset_hash_table();
                self.free_entry = self.clear_code + 2;
                self.clear_flag = true;
                self.output(self.clear_code, output);
            }
        }

        self.output(entry, output);
        self.output(self.eof_code, output);
    }

    fn reset_hash_table(&mut self) {
        self.hash_table = [-1; HASH_SIZE];
    }

    fn next_pixel(&mut self) -> i32 {
        if self.current_pixel < self.pixels.len() {
            let p = self.pixels[self.current_pixel];
            self.current_pixel += 1;
            p as i32
        } else {
            EOF
        }
    }

    fn output<W: Write>(&mut self, code: i32, output: &mut W) {
        self.bit_accumulator &= MASKS[self.bit_count as usize];
        self.bit_accumulator = if self.bit_count > 0 {
            self.bit_accumulator | (code << self.bit_count)
        } else {
            code
        };
        self.bit_count += self.num_bits;

        while self.bit_count >= 8 {
            self.add((self.bit_accumulator & 0xff) as u8, output);
            self.bit_accumulator >>= 8;
            self.bit_count -= 8;
        }

        if self.free_entry > self.max_code || self.clear_flag {
            if self.clear_flag {
                self.num_bits = self.initial_bits;
                self.max_code = max_code(self.num_bits);
                self.clear_flag = false;
            } else {
                self.num_bits += 1;
                self.max_code = if self.num_bits == MAX_BITS {
                    MAX_MAX_CODE
                } else {
                    max_code(self.num_bits)
                };
            }
        }

        if code != self.eof_code {
            return;
        }

        while self.bit_count > 0 {
            self.add((self.bit_accumulator & 0xff) as u8, output);
            self.bit_accumulator >>= 8;
            self.bit_count -= 8;
        }
        self.flush(output);
    }

    fn add<W: Write>(&mut self, value: u8, output: &mut W) {
        self.accumulator[self.char_count] = value;
        self.char_count += 1;
        if self.char_count >= 254 {
            self.flush(output);
        }
    }

    fn flush<W: Write>(&mut self, output: &mut W) {
        if self.char_count == 0 {
            return;
        }

        let _ = output.write_all(&[self.char_count as u8]);
        let _ = output.write_all(&self.accumulator[..self.char_count]);
        self.char_count = 0;
    }
}

fn max_code(num_bits: i32) -> i32 {
    (1 << num_bits) - 1
}
