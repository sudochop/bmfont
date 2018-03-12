use std::io::{Read, BufRead, Cursor, Seek, SeekFrom, Error, ErrorKind};
use std::mem::{transmute, size_of};
use std::collections::HashMap;


macro_rules! parse_error {
	($s: expr) => {
        return Err(Error::new(ErrorKind::InvalidData, $s));
	}
}

macro_rules! parse_many {
    ($buff: ident, $size: expr) => {{
        let mut val: [u8; $size] = [0; $size];
        $buff.read_exact(&mut val)?;
        val
    }};
}

macro_rules! parse_u8 {
    ($buff: ident) => {{
        let mut val: [u8; 1] = [0; 1];
        $buff.read_exact(&mut val)?;
        val[0]
    }};
}

macro_rules! parse_u16 {
    ($buff: ident) => {{
        let mut val: [u8; 2] = [0; 2];
        $buff.read_exact(&mut val)?;
        unsafe { u8_to_u16(val) }
    }};
}

macro_rules! parse_u32 {
    ($buff: ident) => {{
        let mut val: [u8; 4] = [0; 4];
        $buff.read_exact(&mut val)?;
        unsafe { u8_to_u32(val) }
    }};
}

macro_rules! parse_i16 {
    ($buff: ident) => {{
        parse_u16!($buff) as i16
    }};
}

macro_rules! parse_string {
    ($buff: ident) => {{
        let mut string = Vec::new();
        $buff.read_until(0u8, &mut string)?;

        if string.pop().is_none() {
            parse_error!("Invalid string");
        }

        match String::from_utf8(string) {
            Ok(ok) => ok,
            Err(err) => parse_error!(err),
        }
    }};
}


#[inline]
unsafe fn u8_to_u32(a: [u8; 4]) -> u32 {
    transmute::<[u8; 4], u32>(a)
}

#[inline]
unsafe fn u8_to_u16(a: [u8; 2]) -> u16 {
    transmute::<[u8; 2], u16>(a)
}


fn parse_bin(bytes: &[u8]) -> Result<BMFont, Error> {

    let mut buff = Cursor::new(bytes);

    buff.set_position(0);

    if parse_many!(buff, 4) != [66,77,70,3] {
        parse_error!("Missing BMF file identifier");
    }

    // Skip block type and size.
    buff.seek(SeekFrom::Current(5))?;

    // Begin Info block.
    let font_size = parse_i16!(buff);
    let bit_field = parse_u8!(buff);

    let block_info = Info {
        font_size,
        smooth        : bit_field & (1 << 7) != 0,
        unicode       : bit_field & (1 << 6) != 0,
        italic        : bit_field & (1 << 5) != 0,
        bold          : bit_field & (1 << 4) != 0,
        fixed_height  : bit_field & (1 << 3) != 0,
        charset       : parse_u8!(buff),
        stretch_h     : parse_u16!(buff),
        aa            : parse_u8!(buff),
        padding_up    : parse_u8!(buff),
        padding_right : parse_u8!(buff),
        padding_down  : parse_u8!(buff),
        padding_left  : parse_u8!(buff),
        spacing_horiz : parse_u8!(buff),
        spacing_vert  : parse_u8!(buff),
        outline       : parse_u8!(buff),
        font_name     : parse_string!(buff),
    };

    // Skip block type and size.
    buff.seek(SeekFrom::Current(5))?;

    // Begin Common block.
    let block_common = Common {
        line_height : parse_u16!(buff),
        base        : parse_u16!(buff),
        scale_w     : parse_u16!(buff),
        scale_h     : parse_u16!(buff),
        pages       : parse_u16!(buff),
        packed      : parse_u8!(buff) & 1 != 0,
        alpha_chnl  : parse_u8!(buff),
        red_chnl    : parse_u8!(buff),
        green_chnl  : parse_u8!(buff),
        blue_chnl   : parse_u8!(buff),
    };

    // Skip block type and size.
    buff.seek(SeekFrom::Current(5))?;

    // Begin Pages block.
    let mut block_pages: Vec<String> = Vec::with_capacity(block_common.pages as usize);
    for _ in 0..block_common.pages {
        block_pages.push(parse_string!(buff));
    }

    // Skip block type.
    buff.seek(SeekFrom::Current(1))?;

    // Chars block size.
    let size = parse_u32!(buff);

    let total_chars = size / size_of::<Char>() as u32;
    let mut block_chars = Vec::with_capacity(total_chars as usize);

    for _ in 0..total_chars {
        block_chars.push(Char {
            id       : parse_u32!(buff),
            x        : parse_u16!(buff),
            y        : parse_u16!(buff),
            width    : parse_u16!(buff),
            height   : parse_u16!(buff),
            xoffset  : parse_i16!(buff),
            yoffset  : parse_i16!(buff),
            xadvance : parse_i16!(buff),
            page     : parse_u8!(buff),
            chnl     : parse_u8!(buff),
        });
    }

    // Check Kerning block exists.
    let block_kernings = if buff.position() < bytes.len() as u64 {

        // Skip block type.
        buff.seek(SeekFrom::Current(1))?;

        // Chars block size.
        let size = parse_u32!(buff);

        let total_pairs = size / size_of::<Kerning>() as u32;
        let mut pairs_list = Vec::with_capacity(total_pairs as usize);

        for _ in 0..total_pairs {
            pairs_list.push(Kerning {
                first  : parse_u32!(buff),
                second : parse_u32!(buff),
                amount : parse_i16!(buff),
            });
        }

        Some(pairs_list)
    } else {
        None
    };

    // HashMap by character id.
    let mut char_map: HashMap<u32, Char> = HashMap::with_capacity(block_chars.len());
    for c in block_chars {
        char_map.insert(c.id, c);
    }

    Ok(BMFont{
        info: block_info,
        common: block_common,
        pages: block_pages,
        chars: char_map,
        kernings: block_kernings,
    })
}


#[derive(Debug)]
pub struct BMFont {
    pub info: Info,
    pub common: Common,
    pub pages: Vec<String>,
    pub chars: HashMap<u32, Char>,
    pub kernings: Option<Vec<Kerning>>
}


impl BMFont {
    pub fn new(bytes: &[u8]) -> Result<BMFont, Error> {
        parse_bin(bytes)
    }

    pub fn str_to_chars<'a>(&'a self, s: &str) -> Vec<&'a Char> {
        String::from(s)
            .into_bytes()
            .iter()
            .map(|&b| &self.chars[&u32::from(b)])
            .collect()
    }
}


#[derive(Debug)]
pub struct Info {
    pub font_size: i16,
    pub smooth: bool,
    pub unicode: bool,
    pub italic: bool,
    pub bold: bool,
    pub fixed_height: bool,
    pub charset: u8,
    pub stretch_h: u16,
    pub aa: u8,
    pub padding_up: u8,
    pub padding_right: u8,
    pub padding_down: u8,
    pub padding_left: u8,
    pub spacing_horiz: u8,
    pub spacing_vert: u8,
    pub outline: u8,
    pub font_name: String
}


#[derive(Debug)]
pub struct Common {
    pub line_height: u16,
    pub base: u16,
    pub scale_w: u16,
    pub scale_h: u16,
    pub pages: u16,
    pub packed: bool,
    pub alpha_chnl: u8,
    pub red_chnl: u8,
    pub green_chnl: u8,
    pub blue_chnl: u8,
}


#[derive(Debug)]
pub struct Char {
    pub id: u32,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub xoffset: i16,
    pub yoffset: i16,
    pub xadvance: i16,
    pub page: u8,
    pub chnl: u8,
}


#[derive(Debug)]
pub struct Kerning {
    pub first: u32,
    pub second: u32,
    pub amount: i16,
}
